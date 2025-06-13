using EmbedEmul.Binary;
using EmbedEmul.Memory;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    /*
    The table of source statement information generated for a compilation unit consists of a 4-byte
    length followed by a relocated address followed by a list of source statement entries. The 4-byte
    length is the total number of bytes occupied by the source statement information for the
    compilation unit, including the four bytes for the length. The relocated address is the address of
    the first machine instruction generated for that compilation unit.
    A source statement entry contains a source line number (as an unsigned 4-byte integer), a
    statement position within the source line (as an unsigned 2-byte integer) and an address delta (as
    an unsigned 4-byte integer). The special statement position SOURCE_NO_POS has the value
    0xffff, and indicates that the statement entry refers to the entire source line.
     */

    public class DWSourceStatement
    {
        /// <summary>
        /// Inclusive byte length of entry.
        /// </summary>
        internal UInt32 _length;
        public UInt32 Length { get { return _length; } }

        /// <summary>
        /// Address of the first machine instruction generated for compilation unit
        /// </summary>
        internal UInt32 _relocatedAddress;
        public UInt32 Address { get { return _relocatedAddress; } }

        /// <summary>
        /// Statement entries
        /// </summary>
        List<DWSourceStatementEntry> _entries;
        public IEnumerable<DWSourceStatementEntry> Entries { get { return _entries; } }

        public DWSourceStatement(MemoryUnit data, UInt32 lineStart)
        {
            data.CurrentIndex = lineStart;
            _length = data.GetUInt32();
            _relocatedAddress = data.GetUInt32();
            _entries = new List<DWSourceStatementEntry>();

            DWSourceStatementEntry entry;
            do
            {
                entry = new DWSourceStatementEntry();
                entry._lineNumber = data.GetUInt32();
                entry._linePosition = data.GetUInt16();
                entry._addressDelta = data.GetUInt32();
                _entries.Add(entry);
            } while (entry._lineNumber > 0);
        }

        public override string ToString()
        {
            return string.Format("0x{0:X8}_0x{1:X8}", _relocatedAddress, _length);
        }
    }

    public class DWSourceStatementEntry
    {
        /// <summary>
        /// Line number in source file entry refers to.
        /// </summary>
        internal UInt32 _lineNumber;
        public UInt32 LineNumber { get { return _lineNumber; } }
        /// <summary>
        /// Start position in current line this entry refers to...
        /// 0xffff implies entry is for entire line.
        /// </summary>
        internal UInt16 _linePosition;
        public UInt16 LinePosition { get { return _linePosition; } }
        /// <summary>
        /// ?
        /// </summary>
        internal UInt32 _addressDelta;
        public UInt32 AddressDelta { get { return _addressDelta; } }

        public override string ToString()
        {
            if (_linePosition < 0xFFFF)
                return string.Format("<0x{0:X8}> LineNo: {1} Column: {2}", _addressDelta, _lineNumber, _linePosition);
            else
                return string.Format("<0x{0:X8}> LineNo: {1}", _addressDelta, _lineNumber);
        }
    }
}
