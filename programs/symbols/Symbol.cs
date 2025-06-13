using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Tools;
using EmbedEmul.Types;
using System.ComponentModel;
using EmbedEmul.Elf;
using EmbedEmul.Binary;
using EmbedEmul.GTIS;
using System.Diagnostics;
using EmbedEmul.Dwarf;
using System.Runtime.CompilerServices;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.Symbols
{
    public enum SymbolState : byte
    {
        Default,
        Edited,
        Rejected
    }

    [Flags]
    public enum SymbolSource : byte
    {
        Elf = 1,
        Ecfg = 1<<1
    }

    public class Symbol
    {
        public static Symbol Zero = new Symbol();

        public ElfFile Parent { get { return _parent; } }
        internal ElfFile _parent;

        /// <summary>
        /// Symbol ID for use by application
        /// </summary>
        public Int64 ID { get { return _id; } }
        internal Int64 _id;

        /// <summary>
        /// Label for symbol.
        /// </summary>
        public string Label { get { return _label; } }
        internal string _label;

        /// <summary>
        /// Offset into the binary blob where the symbol value is located.
        /// </summary>
        public UInt32 FileAddress { get { return _fileAddress; } }
        internal UInt32 _fileAddress;

        /// <summary>
        /// Address of symbol value at runtime or
        /// value of symbol if of type ABS.
        /// </summary>
        public UInt32 RuntimeAddress { get { return _runtimeAddress; } }
        internal UInt32 _runtimeAddress;

        /// <summary>
        /// Size of symbols value in memory.
        /// </summary>
        public UInt32 Size { get { return _size; } }
        internal UInt32 _size;

        /// <summary>
        /// Type and binding attributes
        /// </summary>
        internal byte _symInfo;
        public SymBinding SymbolBinding { get { return (SymBinding)(_symInfo >> 4); }  set { _symInfo = (byte)(_symInfo & 0xf | (((int)value) << 4)); } }
        public SymType SymbolType { get { return (SymType)(_symInfo & 0xf); } set { _symInfo = (byte)(_symInfo & 0xf0 | ((int)value & 0x0f)); } }

        /// <summary>
        /// Defines a symbols visibility, all other bits are undefined.
        /// </summary>
        internal byte _other;
        public SymVisibility SymbolVisibility { get { return (SymVisibility)(_other & 0x3); } set { _other = (byte)((_other & 0xFC) | (((int)value) & 0x3)); } }

        public SymStorageClass SymbolStorage { get { return _symStorage; } }
        /// <summary>
        /// Boolean indicating whether Symbol is only accessible while program is running
        /// </summary>
        public bool IsRuntimeOnly { get { return (_symStorage & SymStorageClass.ROM) == 0; } }
        internal SymStorageClass _symStorage;

        /// <summary>
        /// Boolean indicating whether Symbol has a type associated with it or if it's a label for another process
        /// </summary>
        public bool IsMetadata { get { return (_symStorage & SymStorageClass.Meta) > 0; } set { if (value) _symStorage |= SymStorageClass.Meta; else _symStorage &= ~SymStorageClass.Meta; } }

        public bool IsProgramObject { get { return (SymbolType == SymType.Function || SymbolType == SymType.Object || SymbolType == SymType.TLS); } }

        public ByteOrder _byteOrder = ByteOrder.Invalid;
        /// <summary>
        /// Symbol type to interpret binary blob into values.
        /// </summary>
        public GenType Type { get { return _type; } }
        internal GenType _type; //root type of Symbol

        public GTISInfo Info { get { return _info; } }
        internal GTISInfo _info;

        public UInt16 SectionIndex { get { return _sectionIndex; } }
        internal UInt16 _sectionIndex;

        public string SectionName
        {
            get
            {
                if (_parent != null)
                {
                    if (_sectionIndex < _parent._sectionHeaders.Length)
                        return _parent._sectionHeaders[_sectionIndex].Name;
                    else
                        return ((ElfSpecialSectionIndex)_sectionIndex).ToString();
                }
                else return "";
            }
        }
        public ElfSectionHeader Section
        {
            get
            {
                if (_parent != null && _sectionIndex < _parent._sectionHeaders.Length)
                    return _parent._sectionHeaders[_sectionIndex];
                else
                    return null;
            }
        }

        internal Int32 _cuIndex;
        public string CompilationUnitName
        {
            get
            {
                if (_parent != null && _cuIndex > -1)
                {
                    return _parent.DwarfContext._cus[_cuIndex]._cuDIE.Label;
                }
                else return "";
            }
        }


        public bool IsValue { get { return (_type is GenBaseValue); } }
        public bool IsCountable { get { return (_type is GenArray); } }
        public long Count { get { return IsCountable ? ((_type as GenArray)._maxCount - (_type as GenArray)._startIndex) : -1; } }
        public string Description { get { return _info != null ? _info.Description : ""; } }

        public Symbol()
        {
            _id = -1;
            _cuIndex = -1;
            _symStorage = SymStorageClass.Meta;
        }

        //public Symbol(
        //    string label,
        //    UInt32 fileAddress,
        //    UInt32 runtimeAddress,
        //    UInt32 size,
        //    GenType type,
        //    bool runtimeOnly,
        //    bool absValue)
        //{
        //    _id = -1;
        //    _label = label;
        //    _fileAddress = fileAddress;
        //    _runtimeAddress = runtimeAddress;
        //    _size = size;
        //    if (type != null && size > 0 && type.ByteSize != size)
        //        type.ByteSize = size;
        //    _type = type;
        //    _runtimeOnly = runtimeOnly;
        //    _absoluteValue = absValue;
        //}

        internal void ExtractElfSymbolInformation(ElfFile parent, MemoryUnit stringTable, MemoryUnit symbolTable)
        {
            _parent = parent;
            _id = -1;
            _cuIndex = -1;
            UInt32 nameIndex = symbolTable.GetUInt32();
            _label = stringTable.GetString(nameIndex, -1);
            _runtimeAddress = symbolTable.GetUInt32();
            _size = symbolTable.GetUInt32();
            _symInfo = symbolTable.GetUInt8();
            _other = symbolTable.GetUInt8();
            _sectionIndex = symbolTable.GetUInt16();

            //INIT_ElfMapFileAddress(); can't go here due to needing information from OTHER Symbols
            //INIT_EvaluateOfflineCharacteristics();
        }

        internal void ConsolidateElfSymbolInformation()
        {
            //if (_label == "application_crc_block_table")
            //    Debug.WriteLine(_label);
            //if (_label == "EECOP_ndl")
            //    Debug.WriteLine(_label);

            INIT_ElfMapFileAddress();
            INIT_ExtractElfTypeInformation();
            //Needs type information
            INIT_EvaluateOfflineCharacteristics();
        }

        /// <summary>
        /// Separate type initialization due to needing access to other Symbols which need to be initialized.
        /// </summary>
        internal void INIT_ExtractElfTypeInformation()
        {
            if (_type == null)
            {
                GenType type = null;
                foreach(DWCompilationUnitHeader cu in _parent.DwarfContext.GetCU(_label))
                {
                    _cuIndex = cu._index;
                    if (cu.TryGetType(_label, _size, out type))
                    {
                        SetType(type, cu._index);
                        break;
                    }
                }
            }
        }

        internal void INIT_ElfMapFileAddress()
        {
            //Find file address
            if (SymbolType != SymType.NoType)
            {
                Symbol initSym;

                if ((ElfSpecialSectionIndex)_sectionIndex == ElfSpecialSectionIndex.SHN_ABS)
                {
                    // Search for actual section of data
                    UInt32 fndSecIdx;
                    if (_parent.TryFindSectionIndex(_runtimeAddress, out fndSecIdx))
                        _sectionIndex = (UInt16)fndSecIdx;
                }

                ElfSectionHeader sec = _parent._sectionHeaders[_sectionIndex];
                //Determine file address (including .abs _init mapping done by OS)
                if (sec._segmentMapping!=null)
                {
                    ElfMapping2 map = sec._segmentMapping;
                    //Added for OS non downloadable trim section (.abs.addr)
                    //Needs to go first due to TriCore linker specifying a file address in ram for the init sections
                    if ((SectionName.StartsWith(".abs") || SectionName.IndexOf(_label, StringComparison.InvariantCultureIgnoreCase) >= 0) &&
                        _parent._symtab.TryGetGlobalSymbol(_label + "_init", out initSym) &&
                        _parent._sectionHeaders[initSym._sectionIndex]._segmentMapping!=null)
                    {
                        ElfMapping2 initMap = _parent._sectionHeaders[initSym._sectionIndex]._segmentMapping;
                        UInt32 mapMemAddr = initMap.MemoryAddress;
                        UInt32 mapFileAddr = initMap.FileAddress;
                        if (initSym._runtimeAddress < mapMemAddr || initSym._runtimeAddress >= mapMemAddr + initMap.MemorySize)
                        {
                            //Init sym is a file or flash address... move to the RAM image.
                            initSym._runtimeAddress = mapMemAddr + initSym._runtimeAddress - mapFileAddr;
                        }
                        _fileAddress = mapFileAddr + initSym._runtimeAddress - mapMemAddr;
                        _symStorage |= SymStorageClass.ROM;
                    }
                    else if (map.FileAddress > 0)
                    {
                        UInt32 mapMemAddr = map.MemoryAddress;
                        UInt32 mapFileAddr = map.FileAddress;
                        if (_runtimeAddress < mapMemAddr || _runtimeAddress >= mapMemAddr + map.MemorySize)
                            _runtimeAddress = mapMemAddr + _runtimeAddress - mapFileAddr;
                        _fileAddress = mapFileAddr + _runtimeAddress - mapMemAddr;
                        _symStorage |= SymStorageClass.ROM;
                    }
                    else _symStorage &= ~SymStorageClass.ROM;
                }
            }
        }

        internal void INIT_EvaluateOfflineCharacteristics()
        {
            ElfSectionHeader section = Section;
            if( //(SymbolBinding == SymBinding.Local) ||
                (SymbolType == SymType.Function) ||
               ((section == null) ||
                (section.SectionType == ElfSection.Type.NOBITS && _fileAddress == 0x00000000) ||
                (section.SectionType == ElfSection.Type.NULL)))
            {
                _symStorage &= ~SymStorageClass.ROM;
            }

            if (_type == null)
                _symStorage |= SymStorageClass.Meta;
            else
                _symStorage &= ~SymStorageClass.Meta;
        }

        internal void SetType(GenType type, Int32 cuIndex = -1)
        {
            _type = type;
            _cuIndex = cuIndex;

            if (_type != null)
            {
                //if (SymbolType == SymType.Function && _type.ByteSize != _size)
                //    Debug.AutoFlush = true;

                if (_size == 0)
                    _size = (UInt32)_type.ByteSize;
                else if (_type.ByteSize != _size && SymbolType != SymType.Function)
                    _type.SetByteSize(_size);

                // Some TriCore compilers (Bosch cals) set syms to NoType even though they have a GenType in the debug info
                if (SymbolType == SymType.NoType)
                {
                    if (_type is GenSubroutine)
                        SymbolType = SymType.Function;
                    else
                        SymbolType = SymType.Object;
                }
                //INIT_ElfMapFileAddress();
                //INIT_EvaluateOfflineCharacteristics();
                _symStorage &= ~SymStorageClass.Meta;
            }
            //else INIT_EvaluateOfflineCharacteristics();
        }

        public override string ToString()
        {
            //Refresh();
            StringBuilder builder = new StringBuilder();
            if (IsMetadata)
                builder.AppendFormat(" LABEL: {0} {1:X8} {2}", _label, _runtimeAddress, _size > 0 ? _size.ToString("X") : "");
            else
            {
                if(SymbolType == SymType.Function)
                    builder.AppendFormat(" FUNC: addr:{0:X8} work:{1:X8} sz:{2:X} {3}", _fileAddress, _runtimeAddress, _size, _label);
                else
                    builder.AppendFormat(" VAR: addr:{0:X8} work:{1:X8} sz:{2:X} {3}", _fileAddress, _runtimeAddress, _size, _label);
            }
            return builder.ToString();
        }
    }

    [Flags]
    public enum SymStorageClass : byte
    {
        ROM = 1<<0, //ROM/File Address Valid
        RAM = 1<<1, //RAM Address Valid
        Meta = 1 << 2, //META meaning has no physical presence but may contain useful data
    }

    public enum SymBinding : byte
    {
        Local = 0,
        Global = 1,
        Weak = 2,
        //10-12 OS defined
        //13-15 Proc defined
    }

    public enum SymType : byte
    {
        NoType = 0,
        Object = 1, //data such as Symbol or array
        Function = 2, //executable code
        Section = 3,
        File = 4,
        Common = 5,
        TLS = 6, //Thread local storage entity
                 //10-12 OS defined
                 //13-15 Proc defined
    }

    public enum SymVisibility : byte
    {
        Default = 0,
        Internal = 1,
        Hidden = 2,
        Protected = 3,
        Exported = 4,
        Singleton = 5,
        Eliminate = 6
    }
}
