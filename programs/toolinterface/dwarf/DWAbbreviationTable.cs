using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public class DWAbbreviationTable
    {
        internal DWAttribute[] _attrCache = new DWAttribute[30];
        internal MemoryUnit _abbrevTable;
        internal Dictionary<UInt64, Dictionary<UInt32, Int64>> _definitionOffsets;

        public DWAbbreviationTable(MemoryUnit abbrevTable)
        {
            _definitionOffsets = new Dictionary<ulong, Dictionary<uint, long>>();
            _abbrevTable = abbrevTable;
            /*
            Need to extract abbreviation tables from debug_abbrev section.  Each table is specified
            by the offset from the start of the debug_abbrev and consists of a number of
            entries terminated by a null entry.
            start
            entry0
            entry1
            entry2
            null
            */

            //debug_abbrev Create dictionary lookup of abbreviation table so that compilation units can supply "abbrev table offset" and get back the
            //corresponding null terminating table of entries
            while (!abbrevTable.EndOfStream)
            {
                var compUnitAbbrevTable = new Dictionary<UInt32, Int64>();
                _definitionOffsets.Add((UInt64)abbrevTable.CurrentAddress, compUnitAbbrevTable);

                ulong abbrevCode = abbrevTable.GetULEB128();
                while (abbrevCode > 0)
                {
                    compUnitAbbrevTable.Add((UInt32)abbrevCode, abbrevTable.CurrentIndex);
                    GoToNextDef(abbrevTable);
                    abbrevCode = abbrevTable.GetULEB128();
                }
            }
        }

        private void GoToNextDef(MemoryUnit abbrev)
        {
            abbrev.GetULEB128(); //skip tag
            abbrev.GetULEB128(); //skip children

            DWAttrType typeCode = (DWAttrType)abbrev.GetULEB128(); //skip first type code
            DWForm formCode = (DWForm)abbrev.GetULEB128(); //skip first form code
            //skip until end
            while (!(typeCode == 0 && formCode == 0))
            {
                typeCode = (DWAttrType)abbrev.GetULEB128();
                formCode = (DWForm)abbrev.GetULEB128();
            }
        }

        public void PopulateDIE(DWDie die, UInt64 tableOffset, UInt32 abbrevCode)
        {
            _abbrevTable.CurrentIndex = _definitionOffsets[tableOffset][abbrevCode];
            die._tag = (DWTag)_abbrevTable.GetULEB128();
            die._children = (DWChildren)_abbrevTable.GetULEB128();

            Int32 count = 0;
            _attrCache[count]._typeCode = (DWAttrType)_abbrevTable.GetULEB128();
            _attrCache[count]._formCode = (DWForm)_abbrevTable.GetULEB128();
            //skip until end
            while (!(_attrCache[count]._typeCode == 0 && _attrCache[count]._formCode == 0))
            {
                count++;
                _attrCache[count]._typeCode = (DWAttrType)_abbrevTable.GetULEB128();
                _attrCache[count]._formCode = (DWForm)_abbrevTable.GetULEB128();
            }

            die._attributeCount = (byte)count;

            if (die._attributes == null || die._attributes.Length < count)
                die._attributes = new DWAttribute[count];

            if (count > 0)
                Array.Copy(_attrCache, die._attributes, count);
        }

        public DWTag GetTag(UInt64 tableOffset, UInt32 abbrevCode)
        {
            _abbrevTable.CurrentIndex = _definitionOffsets[tableOffset][abbrevCode];
            return (DWTag)_abbrevTable.GetULEB128();
        }
    }
}
