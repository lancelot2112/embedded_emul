using EmbedEmul.Types;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Diagnostics;
using EmbedEmul.Binary;
using EmbedEmul.Elf;
using EmbedEmul.Variables;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public class DWDebug
    {
        internal DWAttribute[] _attributeCache;
        internal List<MemoryUnit> _debugInfos;
        internal DWAbbreviationTable _debugAbbrev;
        internal MemoryUnit _debugStr;
        internal MemoryUnit _debugLoc;
        internal List<MemoryUnit> _debugLine;
        internal ElfSectionSymbolTable _symTab;
        internal int _dwarfVersion;

        //internal Dictionary<string, UInt32> _dieByGlobalVariableName;
        //internal Dictionary<string, DwarfDebuggingInformationEntry> _dieByFunctionName;
        //internal Dictionary<UInt32, GenType> _typeAssemblyByOffset;
        //internal Dictionary<string, GenType> _typeAssemblyByName;
        internal ByteOrder _byteOrder;
        internal List<DWCompilationUnitHeader> _cus;
        internal Dictionary<string, HashSet<DWCompilationUnitHeader>> _globalVariableCUs;

        /// <summary>
        /// Flat list of all DIEs
        /// </summary>
        public IEnumerable<DWDie> DIEs { get { return _cus.SelectMany(cu=>cu.GetDIEs()); } }
        //public Dictionary<UInt32, DWDie> DIEMap { get { return _dieByOffset; } }

        public IEnumerable<DWCompilationUnitHeader> CompilationUnits { get { return _cus; } }
        /// <summary>
        /// List of all DIEs without parents.
        /// </summary>
        public IEnumerable<DWDie> CompilationUnitDIEs { get { return _cus.Select(cu => cu._cuDIE); } }
        public long DIECount { get { long count = 0; foreach (DWCompilationUnitHeader cu in _cus) count += cu._dieCount; return count; } }

        public DWDebug(ByteOrder order, int dwarfVersion)
        {
            _byteOrder = order;
            _dwarfVersion = dwarfVersion;
            if (_dwarfVersion == 1)
                _attributeCache = new DWAttribute[30];
            //_typeAssemblyByOffset = new Dictionary<uint, GenType>();
            _debugInfos = new List<MemoryUnit>();
            _debugLine = new List<MemoryUnit>();
            _globalVariableCUs = new Dictionary<string, HashSet<DWCompilationUnitHeader>>(2000);
            //_typeAssemblyByName = new Dictionary<string, GenType>();
            //_dieByGlobalVariableName = new Dictionary<string, UInt32>();
            _cus = new List<DWCompilationUnitHeader>(500);
            //_dieByFunctionName = new Dictionary<string, DwarfDebuggingInformationEntry>();
        }

        public void AddDebugSection(MemoryUnit debugInfo)
        {
            Debug.Assert(debugInfo != null && debugInfo.BufferLength > 0);

            _debugInfos.Add(debugInfo);
        }

        public void SetAbbrevTable(MemoryUnit debugAbbrev)
        {
            Debug.Assert(debugAbbrev != null);
            Debug.Assert(_debugAbbrev == null, "Expected only one .debug_abbrev table per .elf");

            _debugAbbrev = new DWAbbreviationTable(debugAbbrev);
        }

        public void SetStrTable(MemoryUnit debugStr)
        {
            Debug.Assert(debugStr != null);
            Debug.Assert(_debugStr == null, "Expected only one .debug_str table per .elf");
            _debugStr = debugStr;
        }
        internal void SetLocTable(MemoryUnit memoryBlock)
        {
            Debug.Assert(memoryBlock != null);
            Debug.Assert(_debugLoc == null, "Expected only one .debug_loc table per .elf");
            _debugLoc = memoryBlock;
        }

        internal void AddLineTable(MemoryUnit memoryBlock)
        {
            Debug.Assert(memoryBlock != null);
            //Debug.Assert(_debugLine == null, "Expected only one .line or one .debug_line table per .elf");
            _debugLine.Add(memoryBlock);
        }

        DWCompilationUnitHeader mostRecent;
        internal void ExtractMetadata()
        {
            StatusUpdateLog.OnReportProgress("DWARF Context", 0, "Extracting Compilation Units");
            try
            {
                ExtractCUs();
            }
            catch (Exception ex)
            {
                var lastCU = _cus[_cus.Count - 1];
                Debug.WriteLine(string.Format("Exception thrown for compilation header at {0:X}\n{1}\n{2}\n", lastCU._debugInfo.CurrentAddress, ex.Message, ex.StackTrace) + lastCU.ToString());
                throw ex;
            }
            StatusUpdateLog.OnReportProgress("DWARF Context", 1, "Populating Hash Tables");
            try
            {
                PopulateHashTables();
            } catch(Exception ex)
            {
                Debug.WriteLine(string.Format("Exception thrown for compilation header at {0:X}\n{1}\n{2}\n", mostRecent._debugInfo.CurrentAddress, ex.Message, ex.StackTrace) + mostRecent.ToString());
                throw ex;
            }
            StatusUpdateLog.OnReportProgress("DWARF Context", 1, "Finished!");
        }

        internal void PopulateHashTables()
        {
            StatusUpdateLog.OnStartProgress("Hash", (ulong)_cus.Count, "DWARF Context");
            foreach (DWCompilationUnitHeader cu in _cus)
            {
                mostRecent = cu;
                cu.PopulateHashTables();
                StatusUpdateLog.OnReportProgress("Hash", 1, "Populating...", "DWARF Context");
            }
            StatusUpdateLog.OnCloseProgress("Hash", "DWARF Context");
        }

        public void RegisterName(string name, DWCompilationUnitHeader cu)
        {
            HashSet<DWCompilationUnitHeader> cus;
            if (!_globalVariableCUs.TryGetValue(name, out cus))
            {
                cus = new HashSet<DWCompilationUnitHeader>();
                _globalVariableCUs.Add(name, cus);
            }
            cus.Add(cu);
        }

        public IEnumerable<DWCompilationUnitHeader> GetCU(string name)
        {
            HashSet<DWCompilationUnitHeader> cus;
            if (_globalVariableCUs.TryGetValue(name, out cus))
            {
                foreach (DWCompilationUnitHeader cu in cus)
                    yield return cu;
            }
        }

        public IEnumerable<GenType> GetVariableType(string name, UInt64 size)
        {
            GenType type;
            HashSet<DWCompilationUnitHeader> cus;
            if(_globalVariableCUs.TryGetValue(name, out cus))
            {
                foreach (DWCompilationUnitHeader cu in cus)
                {
                    if (cu.TryGetType(name, size, out type))
                    {
                        yield return type;
                    }
                }
            }
        }

        public void ClearCache()
        {
            foreach (DWCompilationUnitHeader cu in _cus)
                cu.ClearCache();

        }
        internal void ExtractCUs()
        {
            Debug.Assert(_debugInfos.Count == _debugLine.Count);
            for (int ii = 0; ii< _debugInfos.Count; ii++)
            {
                var debugInfo = _debugInfos[ii];
                debugInfo.CacheClear();
                while (!debugInfo.EndOfStream)
                {
                    DWCompilationUnitHeader header = new DWCompilationUnitHeader(_cus.Count, this, debugInfo, _debugLine[ii]);
                    if (header._end == header._start)
                        break;
                    _debugInfos[ii].CurrentAddress = header._end;
                    _cus.Add(header);
                }
            }
        }

    }
}
