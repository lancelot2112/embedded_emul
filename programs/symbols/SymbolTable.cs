using EmbedEmul.Dwarf;
using EmbedEmul.Elf;
using EmbedEmul.GTIS;
using EmbedEmul.Types;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Diagnostics;
using System.IO;
using System.Globalization;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.Symbols
{
    /// <summary>
    /// Links Symbols from a variety of sources to create a Symbol table (similar to a symbol table for elf files) but with potentially
    /// more information
    /// </summary>
    public class SymbolTable
    {
        internal ElfFile _elf;
        internal A2LFile _A2L;


        //public event StatusUpdateDelegate StatusUpdate;

        //public void OnStatusUpdate(object owner, string functionName, string message, StatusUpdateType type)
        //{
        //    if (StatusUpdate != null)
        //        StatusUpdate(owner, functionName, message, type);
        //}
        /// <summary>
        /// Returns the minimum trust level of both the A2L and elf.
        /// </summary>
        public TrustLevel TrustLevel
        {
            get
            {
                TrustLevel level = _trustLevel;
                if (_elf != null && _elf.TrustLevel < level)
                    level = _elf.TrustLevel;
                if (_A2L != null && _A2L.TrustLevel < level)
                    level = _A2L.TrustLevel;

                return level;
            }
        }
        internal TrustLevel _trustLevel;

        internal List<Symbol> _Symbols;
        internal Dictionary<string, Int32> _SymbolByLabel;
        internal Dictionary<Int64, Int32> _SymbolById;
        internal Dictionary<string, List<Int32>> _staticSymbolByLabel;
        internal Dictionary<string, GenType> _typesByLabel;
        public IEnumerable<Symbol> Symbols { get { foreach (Symbol Symbol in _Symbols) yield return Symbol; } }
        public IEnumerable<Symbol> ProgramSymbols {  get { foreach (Symbol Symbol in _Symbols.Where(var => var.IsProgramObject)) yield return Symbol;} }
        public IEnumerable<Symbol> GlobalSymbols { get { foreach (Int32 idx in _SymbolByLabel.Values) yield return _Symbols[idx]; } }
        public IEnumerable<Symbol> StaticSymbols { get { foreach (List<Int32> idx_list in _staticSymbolByLabel.Values) { foreach (Int32 idx in idx_list) yield return _Symbols[idx]; } } }
        public IEnumerable<Symbol> FileSymbols { get { return GlobalSymbols.Where(v => !v.IsRuntimeOnly && !v.IsMetadata); } }
        public IEnumerable<Symbol> MetadataSymbols { get { return GlobalSymbols.Where(v => v.IsMetadata); } }
        public IEnumerable<Symbol> RuntimeSymbols { get { return GlobalSymbols.Where(v => v.IsRuntimeOnly); } }
        public IEnumerable<Symbol> CalibratableSymbols { get { return FileSymbols.Where(v => !(v._type is GenPointer)); } }
        public IEnumerable<Symbol> GTISCalibratableSymbols { get { return CalibratableSymbols.Where(v => v._info != null && v._info.IsCalibratable); } }
        public long Count { get { return _SymbolByLabel.Count; } }

        public ElfFile Elf { get { return _elf; } }
        public A2LFile A2L { get { return _A2L; } }
        internal bool _indexTableLinkValid;

        public SymbolTable(Int32 initCount = 50)
            : this(null, null, initCount)
        { }

        public SymbolTable(string A2L, string elf, Int32 initCount = 50)
        {
            _SymbolById = new Dictionary<Int64, Int32>(initCount / 4);
            _SymbolByLabel = new Dictionary<string, Int32>(initCount / 2);
            _staticSymbolByLabel = new Dictionary<string, List<Int32>>(initCount / 6);
            _typesByLabel = new Dictionary<string, GenType>(initCount / 6);
            _Symbols = new List<Symbol>(initCount);
            _trustLevel = TrustLevel.Full;
            _indexTableLinkValid = false;

            if (elf != null)
                AddSymbolsFromElf(elf);

            if (A2L != null)
                AddSymbolsFromA2L(A2L);
        }

        public void AliasSymbol(string alias, string otherAlias)
        {
            int varIdx;
            if (_SymbolByLabel.TryGetValue(otherAlias, out varIdx) &&
              !_SymbolByLabel.ContainsKey(alias))
                _SymbolByLabel.Add(alias, varIdx);
        }

        public void LinkSymbol(Symbol Symbol)
        {
            Symbol original, final;
            Int32 originalIndex;
            if (Symbol.SymbolBinding != SymBinding.Global || !_SymbolByLabel.TryGetValue(Symbol._label, out originalIndex))
            {
                _Symbols.Add(Symbol);
                if (Symbol.SymbolBinding == SymBinding.Global)
                    _SymbolByLabel.Add(Symbol._label, (_Symbols.Count - 1));
                if (Symbol._id > -1)
                    _SymbolById.Add(Symbol._id, (_Symbols.Count - 1));
                if (Symbol.SymbolBinding == SymBinding.Local && (Symbol.SymbolType == SymType.Object || Symbol.SymbolType == SymType.Function))
                {
                    List<Int32> idx_list;
                    if (_staticSymbolByLabel.TryGetValue(Symbol._label, out idx_list))
                        idx_list.Add(_Symbols.Count - 1);
                    else
                    {
                        idx_list = new List<Int32>(1);
                        idx_list.Add(_Symbols.Count - 1);
                        _staticSymbolByLabel.Add(Symbol._label, idx_list);
                    }
                }
                final = Symbol;
            }
            else
            {
                original = _Symbols[originalIndex];
                //original from elf and new from A2L
                if (original._info == null && Symbol._info != null)
                {
                    original._id = Symbol._id;
                    original._info = Symbol._info;

                    //handle table crushing and values defined as hex
                    original._type = _mergeSymbolTypes(Symbol._type, original._type);

                    _SymbolById.Add(original._id, originalIndex);

                } //original from A2L and new from elf
                else if (original._info != null && Symbol._info == null)
                {
                    original._fileAddress = Symbol._fileAddress;
                    original._size = Symbol._size;
                    original._type = _mergeSymbolTypes(original._type, Symbol._type);
                    original._runtimeAddress = Symbol._runtimeAddress;
                    original._symStorage = Symbol._symStorage;
                    original._other = Symbol._other;
                    original._parent = Symbol._parent;
                    original._symInfo = Symbol._symInfo;
                    original._sectionIndex = Symbol._sectionIndex;
                }
                //ignore new one
                final = original;
            }

            GenType genType;
            if (final._type != null)
            {
                if (final._type.Name != null)
                {
                    if (_typesByLabel.TryGetValue(final._type.Name, out genType))
                        _typesByLabel[final._type.Name] = final._type;
                    else
                        _typesByLabel.Add(final._type.Name, final._type);
                }
            }
        }

        private GenType _mergeSymbolTypes(GenType A2LType, GenType elfType)
        {
            if (elfType == null)
                return A2LType;
            else if (elfType is GenPointer)
                return elfType;

            //handle table crushing and values defined as hex
            if (A2LType is GenArray && elfType is GenArray)
            {
                var array = A2LType as GenArray;
                if (!(array._member is GenFixedValue))
                {
                    if (array._member._byteSize != (elfType as GenArray)._member._byteSize)
                        array._member = (elfType as GenArray)._member;
                    else
                        array._member._name = (elfType as GenArray)._member.Name;
                }
                array.SetByteSize(elfType._byteSize);
                elfType = array;
            }
            else if (A2LType is GenBaseValue && elfType is GenBaseValue)
            {
                //Only take A2Ltype information if the same size (A2L more likely to be wrong)
                if (A2LType.ByteSize == elfType.ByteSize)
                {
                    if (!(A2LType is GenFixedValue))
                        A2LType._name = elfType.Name;

                    elfType = A2LType;
                }
            }

            return elfType;
        }

        internal void PerformFinalLink()
        {
            if (_elf != null && _A2L != null)
            {
                Symbol linkSymbol;
                int linkIndex;
                foreach (Symbol Symbol in GlobalSymbols.Where(var => var.Info != null && var.Info.IsInterfaceParameter))
                {
                    long pid = Symbol.ID & 0x00FFFFFF;
                    //Find interface parameters that are directly linked to some other parameter and "merge"
                    if (_SymbolById.TryGetValue(pid, out linkIndex))
                    {
                        linkSymbol = _Symbols[linkIndex];

                        Symbol._symStorage = linkSymbol._symStorage;
                        Symbol._fileAddress = linkSymbol._fileAddress;
                        Symbol._runtimeAddress = linkSymbol._runtimeAddress;
                        Symbol._other = linkSymbol._other;
                        Symbol._size = linkSymbol._size;
                        Symbol._sectionIndex = linkSymbol._sectionIndex;
                        Symbol._symInfo = linkSymbol._symInfo;
                        Symbol._parent = linkSymbol._parent;
                        Symbol._type = linkSymbol._type;
                    }
                }
            }
        }

        public long LinkIndexTable(GTIS.IndexTable indexTable, MemoryManager mem)
        {
            AddressRange range;
            long missCount = 0;
            HashSet<UInt64> indexesFound = new HashSet<UInt64>();
            foreach (Symbol Symbol in GlobalSymbols.Where(var => var._info != null && var._info._isOfflineAccessible))
            {
                if (indexTable._addresses.TryGetValue(Symbol._info.IndexTableNumber, out range))
                {
                    indexesFound.Add(Symbol._info.IndexTableNumber);
                    if (!_indexTableLinkValid)
                    {
                        //Need to set file address because at times the OS does weird things like the .abs sections with the _init params
                        //Elf handling might not always catch these... so make sure we update to the index table value
                        if (Symbol.IsMetadata)
                            Symbol._runtimeAddress = (UInt32)range._start;
                        else if (Symbol._fileAddress == 0x0)
                        {
                            if (mem.TrySeek(ref range) == MemoryManagerState.Valid)
                            {
                                Symbol._fileAddress = (UInt32)range._start;
                                Symbol._symStorage |= SymStorageClass.ROM;
                            }
                        }

                        //Elf size generally more reliable
                        if (_elf == null || Symbol._size == 0)
                            Symbol._size = (UInt32)range._length;

                        //Handle size change due to table crushing
                        if (Symbol._type._byteSize != Symbol._size)
                        {
                            if (Symbol._type is GenArray)
                            {
                                var array = Symbol._type as GenArray;
                                array.SetByteSize(Symbol._size);
                            }
                            else if (!(Symbol._type is GenStructure))
                            {
                                Symbol._type._byteSize = Symbol._size;
                            }
                            //If structure type will have to notify that change in size cannot happen dynamically since
                            //we don't know which parts of the structure to grow or shrink or by how much without more information
                        }
                    }
                }
                else missCount++;
            }

            missCount += indexTable._addresses.Where(kvp => !indexesFound.Contains(kvp.Key)).Count();

            _indexTableLinkValid |= missCount == 0;

            return missCount;
        }

        public bool TryGetSymbolByAddress(UInt64 address, out Symbol Symbol)
        {
            Symbol = null;
            AddressRange lookup = new AddressRange(address, 1);
            List<Symbol> varsFound = _Symbols.Where
                (
                    var => var._type != null &&
                    !var.IsMetadata &&
                    (var.Info == null || !var.Info.IsInterfaceParameter) && //Filter out TIS parameters as they are technically metadata
                    (lookup.Intersects(var._fileAddress, var._size) || lookup.Intersects(var._runtimeAddress, var._size))
                ).ToList();

            if (varsFound.Count == 1)
                Symbol = varsFound[0];

            return Symbol != null;
        }

        public IEnumerable<GenType> GetTypesByName(string search)
        {
            CultureInfo culture = CultureInfo.CurrentCulture;
            foreach(string key in _typesByLabel.Keys)
            {
                if (culture.CompareInfo.IndexOf(key, search, CompareOptions.IgnoreCase) >= 0)
                    yield return _typesByLabel[key];
            }
        }

        public bool TryGetSymbol(string label, out Symbol Symbol)
        {
            Int32 idx;
            if (_SymbolByLabel.TryGetValue(label, out idx))
                Symbol = _Symbols[idx];
            else
                Symbol = null;

            return Symbol != null;
        }

        public bool TryGetSymbol(Int64 id, out Symbol Symbol)
        {
            Int32 idx;
            if (_SymbolById.TryGetValue(id, out idx))
                Symbol = _Symbols[idx];
            else
                Symbol = null;

            return Symbol != null;
        }

        public bool TryGetCalibratableSymbol(string name, out Symbol Symbol)
        {
            Int32 idx;
            if (!_SymbolByLabel.TryGetValue(name, out idx) || //Check if A2L even has parameter
                _Symbols[idx]._info == null ||
                !_Symbols[idx]._info.IsCalibratable)
            {
                Symbol = null;
            }
            else Symbol = _Symbols[idx];

            return Symbol != null;
        }

        public bool TryGetCalibratableSymbol(Int64 parameterId, out Symbol Symbol)
        {
            Int32 idx;
            if (!_SymbolById.TryGetValue(parameterId, out idx) ||
                _Symbols[idx]._info == null ||
                !_Symbols[idx]._info.IsCalibratable)
            {
                Symbol = null;
            }
            else Symbol = _Symbols[idx];

            return Symbol != null;

        }

        public bool TryGetFileSymbol(string name, out Symbol Symbol)
        {
            Int32 idx;
            if (!_SymbolByLabel.TryGetValue(name, out idx) ||
                _Symbols[idx].IsRuntimeOnly)
            {
                Symbol = null;
            }
            else Symbol = _Symbols[idx];

            return Symbol != null;
        }

        public bool TryGetStaticSymbol(string name, out IEnumerable<Symbol> Symbols)
        {
            List<Int32> idx_list;
            if (_staticSymbolByLabel.TryGetValue(name, out idx_list))
            {
                Symbols = _staticEnumerator(idx_list);
            }
            else Symbols = null;

            return Symbols != null;

        }

        private IEnumerable<Symbol> _staticEnumerator(IEnumerable<Int32> idx_list)
        {
            foreach (Int32 idx in idx_list)
                yield return _Symbols[idx];
        }
        public bool TryGetFileSymbol(Int64 parameterId, out Symbol Symbol)
        {
            Int32 idx;
            if (!_SymbolById.TryGetValue(parameterId, out idx) ||
                _Symbols[idx].IsRuntimeOnly)
            {
                Symbol = null;
            }
            else Symbol = _Symbols[idx];

            return Symbol != null;
        }


        public IEnumerable<Symbol> GetFileSymbolsBetweenLabels(IEnumerable<LabelRange> lblRanges)
        {

            foreach (LabelRange lblRange in lblRanges)
            {
                foreach (Symbol Symbol in GetFileSymbolsBetweenLabels(lblRange))
                    yield return Symbol;
            }
        }

        public IEnumerable<Symbol> GetFileSymbolsBetweenLabels(LabelRange lblRange)
        {
            AddressRange range;
            if (lblRange.TryGetRange(this, out range))
            {
                foreach (Symbol Symbol in FileSymbols.Where(v => range.Contains(v._fileAddress, v._size)))
                    yield return Symbol;
            }
        }

        public IEnumerable<Symbol> GetFileSymbolsInSection(string section)
        {
            Int64 sectionIndex = _elf.GetSectionIndex(section);
            if (sectionIndex > 0)
            {
                foreach (Symbol var in FileSymbols.Where(v =>
                    v.Section.Index == sectionIndex))
                {
                    yield return var;
                }
            }
        }

        public void AddFromFile(string filePath)
        {
            GTISFile file;
            GTISType type;
            if (GTISDataFactory.TryGetFileType(filePath, out type))
            {
                if (type == GTISType.Configuration && _A2L == null)
                {
                    GTISDataFactory.TryGetFile(GTISType.Configuration, filePath, out file, this);
                }
                else if (type == GTISType.Executable && _elf == null)
                {
                    GTISDataFactory.TryGetFile(GTISType.Executable, filePath, out file, this);
                }
            }
        }

        public void AddSymbolsFromA2L(string A2LPath)
        {
            GTISFile A2L;
            if (_A2L == null)
            {
                GTISDataFactory.TryGetFile(A2LPath, out A2L, this);
                _A2L = A2L as A2LFile;
            }
        }
        public void AddSymbolsFromA2L(A2LFile A2L)
        {
            if (_A2L == null)
            {
                foreach (Symbol Symbol in A2L.Symbols)
                    LinkSymbol(Symbol);

                _A2L = A2L;
                PerformFinalLink();
                _A2L._SymbolTable = this;
            }
        }

        public void AddSymbolsFromElf(string elfPath)
        {
            GTISFile elf;
            if (_elf == null)
            {
                GTISDataFactory.TryGetFile(elfPath, out elf, this);
                _elf = elf as ElfFile;
            }
        }
        public void AddSymbolsFromElf(ElfFile elf)
        {
            if (_elf == null && elf.SymbolTable != null)
            {
                var oldSymbols = _Symbols;

                _elf = elf;
                var table = _elf.SymbolTable.SymbolTable;
                _Symbols = table._Symbols;
                _SymbolById = table._SymbolById;
                _SymbolByLabel = table._SymbolByLabel;
                _staticSymbolByLabel = table._staticSymbolByLabel;
                _typesByLabel = table._typesByLabel;

                if (oldSymbols != null)
                {
                    foreach (Symbol Symbol in oldSymbols)
                    {
                        LinkSymbol(Symbol);
                    }

                    PerformFinalLink();
                }
                _elf.SymbolTable._SymbolTable = this;
            }
        }

        public bool TryGetCalibratableSymbol(object label, out Symbol Symbol)
        {
            throw new NotImplementedException();
        }
    }

    public enum LabelEndType : byte
    {
        Size,
        Inclusive,
        Exclusive
    }
    public class LabelRange
    {
        public string StartLabel;
        public string EndLabel;
        public LabelEndType EndType;

        public LabelRange()
        { }
        public LabelRange(string start, string end)
        {
            StartLabel = start;
            EndLabel = end;
            EndType = LabelEndType.Exclusive;
        }
        public LabelRange(string start, string end, LabelEndType type)
        {
            StartLabel = start;
            EndLabel = end;
            EndType = type;
        }

        public bool TryGetRange(SymbolTable table, out AddressRange range)
        {
            range = new AddressRange();
            Symbol start, end;
            if (table.TryGetSymbol(StartLabel, out start) && table.TryGetSymbol(EndLabel, out end))
            {
                range._start = start._runtimeAddress;
                if (EndType == LabelEndType.Size)
                    range.Length = end._runtimeAddress;
                else if (EndType == LabelEndType.Inclusive)
                    range.InclusiveEnd = end._runtimeAddress;
                else
                    range.ExclusiveEnd = end._runtimeAddress;
                return true;
            }
            return false;
        }
    }

    //TODO: Utilize a value cache in the compares or the overlay process to cache the calculated values for use in other processes
    public class ValueCache
    {
        internal OrderedCache<NumericRegister> _numericCache;
        public OrderedCache<NumericRegister> Numeric { get { return _numericCache; } }
        internal OrderedCache<string> _stringCache;
        public OrderedCache<string> String { get { return _stringCache; } }

        public ValueCache()
        {
            _numericCache = new OrderedCache<NumericRegister>();
            _stringCache = new OrderedCache<string>();
        }

        public void Clear()
        {
            _numericCache.Clear();
            _stringCache.Clear();
        }
        public void Reset()
        {
            _numericCache.Reset();
            _stringCache.Reset();
        }
    }

    public class OrderedCache<T>
    {
        internal List<T> _cache;
        internal Int32 _index;
        internal bool _hasValue;

        public void Clear()
        {
            _cache.Clear();
            _index = -1;
            _hasValue = false;
        }

        public void Reset()
        {
            _index = -1;
            _hasValue = false;
        }

        public void Add(T item)
        {
            _cache.Add(item);
        }
        public T Current()
        {
            if (_hasValue)
                return _cache[_index];
            else return default(T);
        }

        public bool Next(out T value)
        {
            _index++;
            _hasValue = _index != -1 && _index < _cache.Count;
            if (_hasValue)
                value = _cache[_index];
            else value = default(T);

            return _hasValue;
        }
    }


}
