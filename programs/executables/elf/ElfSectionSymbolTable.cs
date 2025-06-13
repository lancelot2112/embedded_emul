using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.GTIS;
using System.Diagnostics;
using EmbedEmul.Variables;
using EmbedEmul.Memory;

namespace EmbedEmul.Elf
{
    public class ElfSectionSymbolTable
    {
        internal ElfFile _parent;
        internal ElfSectionHeader _header;
        internal MemoryUnit _symbolTable;
        internal MemoryUnit _stringTable;
        public UInt32 SymbolCount { get { return _header._size / _header._entitySize; } }
        internal VariableTable _variableTable;
        public VariableTable VariableTable { get { if (_variableTable == null) InitSymbols(); return _variableTable; } }
        public IEnumerable<Variable> Symbols { get { return GetSymbols(); } }
        public IEnumerable<Variable> GlobalSymbols { get { return GetGlobalSymbols(); } }

        public ElfSectionSymbolTable(ElfFile parent, MemoryUnit sectionData, ElfSectionHeader header, MemoryUnit stringTable)
        {
            _parent = parent;
            _header = header;
            _symbolTable = sectionData;
            _stringTable = stringTable;
        }

        internal void InitSymbols()
        {
            uint symbolCount = SymbolCount;
            UInt64 end = (UInt64)_symbolTable.BufferLength;
            UInt64 symStart = 0;

            /* Create the variable table to hold the symbols */
            _variableTable = new VariableTable((Int32)SymbolCount);
            _variableTable._elf = _parent;

            //Find all global symbols
            /*
                0-_nameIndex = symbolTable.GetUInt32();
                _name = stringTable.GetString(_nameIndex, -1);
                4-_value = symbolTable.GetUInt32();
                8-_size = symbolTable.GetUInt32();
                12-_info = symbolTable.GetUInt8();
                13-_other = symbolTable.GetUInt8();
                14-_sectionHeaderIndex = symbolTable.GetUInt16();

                public SymBinding SymbolBinding { get { return (SymBinding)(_info >> 4); } }
                public SymType SymbolType { get { return (SymType)(_info & 0xf); } }
                public SymVisibility SymbolVisibility { get { return (SymVisibility)(_other & 0x3); } }
            */
            _symbolTable.CacheClear();
            Int32 syncId;
            for (; symStart < end; symStart += _header._entitySize)
            {
                syncId = _symbolTable.CacheIndex((long)symStart);
                var variable = new Variable();
                //Extract all the information that can be gathered independently
                variable.ExtractElfSymbolInformation(_parent, _stringTable, _symbolTable);
                _symbolTable.DesyncCacheRange(syncId);

                _variableTable.LinkVariable(variable);
            }

            /* TODO: Make sure type information is available (ie. DWARF is present) */
            foreach (Variable variable in _variableTable.Variables)
            {
                //Run over all variables again now that all has tables are
                //initialized and get any XREF information
                variable.ConsolidateElfSymbolInformation();
            }

            _parent.ClearCache();
        }

        public IEnumerable<Variable> GetSymbols()
        {
            if (_variableTable == null)
                InitSymbols();

            return _variableTable.Variables;
        }

        public IEnumerable<Variable> GetGlobalSymbols()
        {
            if (_variableTable == null)
                InitSymbols();

            return _variableTable.GlobalVariables;
        }

        public bool TryGetGlobalSymbol(string name, out Variable symbol)
        {
            if (_variableTable == null)
                InitSymbols();

            return _variableTable.TryGetVariable(name, out symbol);
        }
    }
}
