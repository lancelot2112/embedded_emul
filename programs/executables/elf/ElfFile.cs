using EmbedEmul.Dwarf;
using EmbedEmul.Elf;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.IO;
using EmbedEmul.Tools;
using EmbedEmul.Variables;
using EmbedEmul.Binary;
using EmbedEmul.Types;
using EmbedEmul.Memory;

namespace EmbedEmul.GTIS
{
    /* 32-Bit Data Types
     * Elf32_Addr Unsigned program address
     * Elf32_Half Unsigned medium integer
     * Elf32_Off Unsigned file offset
     * Elf32_Sword Signed large integer
     * Elf32_Word Unsigned large integer
     * unsigned char Unsigned small integer
     */

    public class ElfFile : GTISFile
    {
        public static HashSet<string> VALID_EXTENSIONS = new HashSet<string>() { ".elf", ".out", ".o", ".so", ".lib", ".a" };

        public override string Description { get { return Name; } }

        internal ElfHeader _header;
        public ElfHeader Header { get { return _header; } }
        internal ElfSegmentHeader[] _segmentHeaders;
        public ElfSegmentHeader[] SegmentHeaders { get { return _segmentHeaders; } }
        internal ElfSectionHeader[] _sectionHeaders;
        public ElfSectionHeader[] SectionHeaders { get { return _sectionHeaders; } }

        internal BinaryImage _fileImage;
        public BinaryImage FileImage { get { return _fileImage; } }

        internal ElfSectionSymbolTable _symtab;
        internal ElfSectionSymbolTable _dynsymtab;
        public ElfSectionSymbolTable SymbolTable { get { return _symtab; } }

        //internal Dictionary<UInt32, ElfMapping> _sectionSegmentMapping;
        //public Dictionary<UInt32, ElfMapping> SectionMapping { get { return _sectionSegmentMapping; } }
        //internal Dictionary<UInt32, List<ElfMapping>> _segmentSectionMapping;
        //public Dictionary<UInt32, List<ElfMapping>> SegmentMapping { get { return _segmentSectionMapping; } }

        //only one section guaranteed REF: http://docs.oracle.com/cd/E19620-01/805-4693/6j4emccrq/index.html
        internal UInt32 _symtabIdx;
        internal UInt32 _dynsymtabIdx;
        internal UInt32 _hashIdx;
        internal UInt32 _dynIdx;
        internal UInt32 _lastFoundIdx;

        private int _dwarfVersion;
        public MemoryUnit GetSection(string name)
        {
            for (int ii = 0; ii < _sectionHeaders.Length; ii++)
            {
                if (_sectionHeaders[ii]._name.Equals(name))
                    return _sectionHeaders[ii].SectionData;
            }
            return null;
        }
        private DWDebug _dwarfContext;
        public DWDebug DwarfContext
        {
            get
            {
                if (_dwarfContext == null)
                {
                    _dwarfContext = new DWDebug(_header.ByteOrder, _dwarfVersion);
                    if (_dwarfVersion == 1)
                    {
                        for (int ii = 0; ii < _sectionHeaders.Length; ii++)
                        {
                            if (!string.IsNullOrEmpty(_sectionHeaders[ii]._name))
                            {
                                switch (_sectionHeaders[ii]._name)
                                {
                                    case ".debug":
                                        _dwarfContext.AddDebugSection(_sectionHeaders[ii].SectionData);
                                        break;
                                    case ".line":
                                        _dwarfContext.AddLineTable(_sectionHeaders[ii].SectionData);
                                        break;
                                }
                            }
                        }
                    }
                    else if (_dwarfVersion >= 2)
                    {
                        for (int ii = 0; ii < _sectionHeaders.Length; ii++)
                        {
                            if (!string.IsNullOrEmpty(_sectionHeaders[ii]._name))
                            {
                                switch (_sectionHeaders[ii]._name)
                                {
                                    case ".debug_info":
                                        _dwarfContext.AddDebugSection(_sectionHeaders[ii].SectionData);
                                        break;
                                    case ".debug_abbrev":
                                        _dwarfContext.SetAbbrevTable(_sectionHeaders[ii].SectionData);
                                        break;
                                    case ".debug_str":
                                        _dwarfContext.SetStrTable(_sectionHeaders[ii].SectionData);
                                        break;
                                    case ".debug_loc":
                                        _dwarfContext.SetLocTable(_sectionHeaders[ii].SectionData);
                                        break;
                                    case ".debug_type":
                                        throw new NotImplementedException();
                                    case ".debug_line":
                                        _dwarfContext.AddLineTable(_sectionHeaders[ii].SectionData);
                                        break;
                                }
                            }
                        }
                    }
                    _dwarfContext._symTab = _symtab;
                    StatusUpdateLog.OnStartProgress("DWARF Context", 2);
                    //Extract all the metadata we need for fast lookup
                    _dwarfContext.ExtractMetadata();
                    StatusUpdateLog.OnCloseProgress("DWARF Context");
                }
                return _dwarfContext;
            }
        }

        //internal ElfSectionText _text;

        public ElfFile(string path, StatusUpdateDelegate statusHandlers = null)
        {
            if (statusHandlers != null)
                StatusUpdate += statusHandlers;

            _trustLevel = TrustLevel.Full;
            Read(path);

            _lastWriteTime = _fileInfo.LastWriteTime;
        }

        public void ClearCache()
        {
            if (_dwarfContext != null)
                _dwarfContext.ClearCache();
        }

        public bool TryFindSectionIndex(UInt32 address, out UInt32 index)
        {
            //Shortcut last found index (hopefully this speeds up the searching unless they come in random order)
            index = (UInt32)ElfSpecialSectionIndex.SHN_UNDEF;
            if (_lastFoundIdx != (UInt32)ElfSpecialSectionIndex.SHN_UNDEF)
            {
                ElfSectionHeader lastSecHdr = _sectionHeaders[_lastFoundIdx];
                if (lastSecHdr._address <= address && (lastSecHdr._address + lastSecHdr._size) > address)
                {
                    index = _lastFoundIdx;
                    return true;
                }
            }

            foreach (ElfSectionHeader secHdr in _sectionHeaders)
            {
                if (secHdr._address <= address && (secHdr._address + secHdr._size) > address)
                {
                    _lastFoundIdx = secHdr._index;
                    index = _lastFoundIdx;
                    break;
                }
            }
            return index != (UInt32)ElfSpecialSectionIndex.SHN_UNDEF;
        }

        public bool TryGetRawSymbolData(string symbolName, out IEnumerable<byte> rawData)
        {
            Variable sym;
            rawData = null;
            if (_symtab.TryGetGlobalSymbol(symbolName, out sym))
            {
                ElfSectionHeader section = sym.Section;
                if (section != null && section.SectionType != ElfSection.Type.NULL && section.SectionType != ElfSection.Type.NOBITS)
                {
                    if (_fileImage.Memory.ViewVar(sym).State == AccessorState.None)
                        rawData = _fileImage.Memory.GetBytes();
                }
            }

            return rawData != null;
        }

        private BinaryImage ReadFileImage(System.IO.BinaryReader reader, ElfSegmentHeader[] segmentHeaders)
        {

            Debug.Assert(segmentHeaders != null && segmentHeaders.Length > 0);

            //Combine segment ranges into contiguous ranges to get the minimum number of blocks to read in for fileSize > 0 blocks
            MemoryManager manager = new MemoryManager();
            foreach (var seg in segmentHeaders.Where(h => h._fileSize > 0))
            {
                UInt64 address = seg._physicalAddress > 0 ? seg._physicalAddress : seg._virtualAddress;
                reader.BaseStream.Seek(_header._elfHeaderOffset + (long)seg._offset - reader.BaseStream.Position, System.IO.SeekOrigin.Current);
                //if (manager.TrySeek(address) != MemoryManagerState.ValidNoValue)
                //    throw new NotImplementedException("wasn't ready for this.");
                manager.FillBytes(reader.ReadBytes((int)seg._fileSize), seg._fileSize);
            }

            return new BinaryImage(_fileInfo.FullName, manager.CachedBlocks.OrderBy(f => f.Range._start).ToList());
        }

        public bool GetCodeCRC16(out UInt16 result, BinaryImage fileToCrc = null)
        {
            result = 0;
            AddressRange textRange = new AddressRange();
            MemoryUnit data;

            if (_symtab == null) return false;
            bool crcThisElf = fileToCrc == null;
            if (crcThisElf) fileToCrc = _fileImage;

            foreach (Variable symbol in _symtab.GlobalSymbols.Where(sym => sym.SymbolType == SymType.Function && sym.SymbolBinding == SymBinding.Global))
            {
                //Skip over symbols who have symbol information but weren't linked in the final assembly?
                if (symbol._size == 0) continue;

                textRange._start = symbol._fileAddress;
                textRange._length = symbol._size;
                if (crcThisElf)
                {
                    //When CRCing the .elf file all our subroutine symbols should be in the file and defined
                    //Throw an exception if this assumption isn't met.
                    if (fileToCrc.Memory.RawMemory.TrySeek(ref textRange) == MemoryManagerState.Valid)
                        data = fileToCrc.Memory.RawMemory.WorkingBlock;
                    else
                        throw new NullReferenceException("Shouldn't get here.");
                }
                else
                {
                    //When crcing another file we aren't sure that it contains all oru information
                    //so just return false if we determine there isn't an expected memory block.
                    if (fileToCrc.Memory.RawMemory.TrySeek(ref textRange) == MemoryManagerState.Valid)
                        data = fileToCrc.Memory.RawMemory.WorkingBlock;
                    else
                        return false;
                }

                //result = (ushort)data.GetChecksum(textRange.Length, result);
                result = data.GetCRC16(textRange.Length, result);
            }
            return true;
        }

        public IEnumerable<Variable> GetCodeDifferences(BinaryImage fileToCompare)
        {
            if (fileToCompare == null) throw new ArgumentNullException("fileToCompare");

            AddressRange textRange = new AddressRange();
            IEnumerator<byte> calData, elfData;
            foreach (Variable symbol in _symtab.GlobalSymbols.Where(sym => sym.SymbolType == SymType.Function && sym.SymbolBinding == SymBinding.Global))
            {
                textRange._start = symbol._fileAddress;
                textRange._length = symbol._size;

                if (_fileImage.Memory.RawMemory.TrySeek(ref textRange) == MemoryManagerState.Valid)
                    elfData = _fileImage.Memory.RawMemory.WorkingBlock.GetBytes().GetEnumerator();
                else throw new NullReferenceException("Shouldn't get here.");

                if (fileToCompare.Memory.RawMemory.TrySeek(ref textRange) == MemoryManagerState.Valid)
                {
                    calData = fileToCompare.Memory.RawMemory.WorkingBlock.GetBytes().GetEnumerator();
                    while(elfData.MoveNext())
                    {
                        if (!calData.MoveNext() || calData.Current != elfData.Current)
                        {
                            Debug.WriteLine($"{symbol.Label} - {symbol.FileAddress:X8}_{symbol.Size:X} elf:{elfData.Current:X2} vs cal:{calData.Current:X2} @{_fileImage.Memory.RawMemory.WorkingBlock.CurrentAddress:X8}");
                            yield return symbol;
                            break;
                        }
                    }
                }
                else yield return symbol;
            }
        }

        public DateTime GetDateStamp()
        {
            return default(DateTime);
        }

        public IEnumerable<AddressRange> CalculateIdentifier()
        {
            List<AddressRange> contiguousRanges = new List<AddressRange>();
            AddressRange currentRange = new AddressRange();
            bool first = true;
            uint segAddress;
            foreach (ElfSegmentHeader seg in _segmentHeaders.Where(s => s.FileSize > 0).OrderBy(s => s.VirtualAddress).OrderBy(s => s.PhysicalAddress))
            {
                segAddress = seg.PhysicalAddress > 0 ? seg.PhysicalAddress : seg.VirtualAddress;
                if (first)
                {
                    first = false;
                    currentRange.MoveTo(segAddress);
                }

                if (currentRange.ExclusiveEnd == segAddress)
                    currentRange.Length += seg.FileSize;
                else
                {
                    //contiguousRanges.Add(currentRange);
                    yield return currentRange;
                    currentRange.MoveTo(segAddress);
                    currentRange.Length = seg.FileSize;
                }
            }
        }

        public bool TryGetFileAddress(UInt64 address, out UInt64 fileAddress)
        {
            ElfMapping2 map;
            fileAddress = 0;
            bool found = false;
            foreach (ElfSectionHeader sec in _sectionHeaders)
            {
                if (sec._segmentMapping == null) continue;

                map = sec._segmentMapping;
                UInt32 mapFileAddr = map.FileAddress;
                UInt32 mapMemAddr = map.MemoryAddress;
                if (mapFileAddr <= address && mapFileAddr + map.FileSize > address)
                {
                    fileAddress = address;
                    found = true;
                    break;
                }
                else if (mapMemAddr <= address && mapMemAddr + map.MemorySize > address)
                {
                    if (sec.SectionType != ElfSection.Type.NULL ||
                        sec.SectionType != ElfSection.Type.NOBITS)
                    {
                        fileAddress = mapFileAddr + address - mapMemAddr;
                        found = true;
                    }

                    break;
                }
            }

            return found;
        }

        public Int64 GetSectionIndex(string name)
        {
            Int64 index;
            for (index = _sectionHeaders.Length - 1; index >= 0; index--)
            {
                if (_sectionHeaders[index]._name.Equals(name))
                    break;
            }

            //Will return 0 if not found (null section)
            return index;
        }

        private void Read(string path)
        {
            _fileInfo = new FileInfo(path);
            long fileSize = _fileInfo.Length;
            using (var file = _fileInfo.OpenRead())
            using (var reader = new System.IO.BinaryReader(file))
            {

                if (fileSize < ElfHeader.TOTAL_LEN)
                {
                    _trustLevel = TrustLevel.Error;
                    OnStatusUpdate(this, "ElfFile.Read", "File length is shorter than expected, check file size.  Could be a sparse file.", StatusUpdateType.Error);
                }
                _header = new ElfHeader(reader);

                if (_header._elfHeaderOffset < 0 ||
                    _header.ByteOrder == ByteOrder.Invalid ||
                    _header.Class > ElfProcessorClass.Address_64Bit ||
                    _header.Class == ElfProcessorClass.Invalid)
                {
                    _trustLevel = TrustLevel.Error;
                    return; //Not valid elf
                }

                //Header or Segment count went over the default 65535 number of entries
                //if (_header._segmentHeaderEntryCount == 65535)

                //get the null section header which will contain the section header and segment header counts if they go over 65535
                if (reader.BaseStream.Position != _header._sectionHeaderOffset)
                    reader.BaseStream.Seek(_header._elfHeaderOffset + _header._sectionHeaderOffset - reader.BaseStream.Position, System.IO.SeekOrigin.Current);
                ElfSectionHeader nullHdr = new ElfSectionHeader(reader, _header.ByteOrder, 0);
                //If the number of sections is greater than or equal to SHN_LORESERVE (0xff00), e_shnum has the value SHN_UNDEF (0) and the actual number of section header
                // table entries is contained in the sh_size field of the section header at index 0 (otherwise, the sh_size member of the initial entry contains 0).
                if (_header._sectionHeaderEntryCount == 0)
                    _header._sectionHeaderEntryCount = nullHdr.Size;
                //If the number of segments is greater than 0xff00 then ...
                if (_header._segmentHeaderEntryCount == 0xFFFF && nullHdr.Info > 0)
                    _header._segmentHeaderEntryCount = nullHdr.Info;

                //Check file size
                if (_header._segmentHeaderEntryCount == 0xFFFF ||
                    _header._sectionHeaderEntryCount == 0 ||
                    (fileSize < _header._segmentHeaderOffset + _header._segmentHeaderEntrySize * _header._segmentHeaderEntryCount) ||
                    (fileSize < _header._sectionHeaderOffset + _header._sectionHeaderEntrySize * _header._sectionHeaderEntryCount))
                {

                    _trustLevel = TrustLevel.Error;
                    OnStatusUpdate(this, "ElfFile.Read", "File length is shorter than expected, check file size.  Could be a sparse file.", StatusUpdateType.Error);
                    return;
                }

                // Defines executable memory image
                StatusUpdateLog.OnStartProgress(Name, 5);
                StatusUpdateLog.OnReportProgress(Name, 0, "Reading Segment Table Headers...");
                if (_header._segmentHeaderOffset != 0)
                {

                    if (reader.BaseStream.Position != _header._segmentHeaderOffset)
                        reader.BaseStream.Seek(_header._elfHeaderOffset + _header._segmentHeaderOffset - reader.BaseStream.Position, System.IO.SeekOrigin.Current);

                    _segmentHeaders = new ElfSegmentHeader[_header._segmentHeaderEntryCount];
                    for (int ii = 0; ii < _header._segmentHeaderEntryCount; ii++)
                    {
                        _segmentHeaders[ii] = new ElfSegmentHeader(reader, _header.ByteOrder, (uint)ii);
                    }

                    _fileImage = ReadFileImage(reader, _segmentHeaders);
                    //_segmentData = new MemoryUnit[_header._segmentHeaderEntryCount];
                    //for (int ii = 0; ii < _header._segmentHeaderEntryCount; ii++)
                    //{
                    //    if (_segmentHeaders[ii].Type != ElfSegment.Type.Null && _segmentHeaders[ii]._fileSize > 0)
                    //    {
                    //        reader.BaseStream.Seek(_header._elfHeaderOffset + _segmentHeaders[ii]._offset, System.IO.SeekOrigin.Begin);
                    //        _segmentData[ii] = new MemoryBlock((UInt64)ii, reader.ReadBytes((int)_segmentHeaders[ii]._fileSize), _header.ByteOrder, (UInt64)_segmentHeaders[ii]._physicalAddress);
                    //    }
                    //}

                }
                else
                {
                    _segmentHeaders = new ElfSegmentHeader[0];
                }

                StatusUpdateLog.OnReportProgress(Name, 1, "Reading Section Table Headers...");
                //Defines linker information
                UInt64 minSecHdrOffset = UInt64.MaxValue;
                if (_header._sectionHeaderOffset != 0)
                {
                    if (reader.BaseStream.Position != _header._sectionHeaderOffset)
                        reader.BaseStream.Seek(_header._elfHeaderOffset + _header._sectionHeaderOffset - reader.BaseStream.Position, System.IO.SeekOrigin.Current);

                    _sectionHeaders = new ElfSectionHeader[_header._sectionHeaderEntryCount];
                    for (int ii = 0; ii < _header._sectionHeaderEntryCount; ii++)
                    {
                        _sectionHeaders[ii] = new ElfSectionHeader(reader, _header.ByteOrder, (uint)ii);
                        if (_sectionHeaders[ii]._offset != 0 && _sectionHeaders[ii]._offset < minSecHdrOffset)
                        {
                            minSecHdrOffset = _sectionHeaders[ii]._offset;
                        }
                    }

                    StatusUpdateLog.OnReportProgress(Name, 1, "Reading Special Section Contents...");
                    foreach (var sec in _sectionHeaders.Where(h => h.SectionType != ElfSection.Type.NOBITS && h._address == 0 && h._size > 0).OrderBy(h => h._offset))
                    {
                        reader.BaseStream.Seek(_header._elfHeaderOffset + sec._offset, System.IO.SeekOrigin.Begin);
                        sec.SectionData = new MemoryUnit((UInt64)sec._index, reader.ReadBytes((int)sec._size), _header.ByteOrder, (UInt64)sec._address);
                        switch (sec.SectionType)
                        {  //Only one section of following types are guaranteed
                            case ElfSection.Type.SYMTAB:
                                Debug.Assert(_symtabIdx == 0);
                                _symtabIdx = (UInt32)sec._index;
                                break;
                            case ElfSection.Type.DYNSYMTAB:
                                Debug.Assert(_dynsymtabIdx == 0);
                                _dynsymtabIdx = (UInt32)sec._index;
                                break;
                            case ElfSection.Type.DYN:
                                Debug.Assert(_dynIdx == 0);
                                _dynIdx = (UInt32)sec._index;
                                break;
                            case ElfSection.Type.HASH:
                                Debug.Assert(_hashIdx == 0);
                                _hashIdx = (UInt32)sec._index;
                                break;
                            default:
                                break;
                        }
                    }

                    //Get the string table section that defines section names
                    StatusUpdateLog.OnReportProgress(Name, 1, "Pulling Section Names from String Table...");
                    if (_header._sectionHeaderNameStringTableIndex != 0) //SHN_UNDEF == 0
                    {
                        MemoryUnit shstrtab = _sectionHeaders[_header._sectionHeaderNameStringTableIndex].SectionData;
                        Debug.Assert(shstrtab != null);
                        for (UInt32 ii = 0; ii < _header._sectionHeaderEntryCount; ii++)
                        {
                            _sectionHeaders[ii]._name = shstrtab.GetString(_sectionHeaders[ii]._nameIndex, -1);
                            if (_dwarfVersion == 0)
                            {
                                if (_sectionHeaders[ii]._name.Equals(".debug"))
                                {
                                    /*
                                        .debug_srcinfo - Lookup table for source information taken from .line
                                        .line - Line number information
                                        .debug_sfnames - string table lookup for source information
                                        .debug - dwarf 1.1 debug section
                                        */
                                    _dwarfVersion = 1; //Dwarf version 1.1
                                }
                                else if (_sectionHeaders[ii]._name.Equals(".debug_info"))
                                {
                                    /*
                                        .debug_aranges - Lookup table for mapping addresses to compilation units
                                        .debug_pubnames - Lookup table for global objects and functions
                                        .debug_pubtypes - Lookup table for global types
                                        .debug_abbrev - Abbreviations used in the .debug_info section
                                        .debug_info - Core DWARF information section
                                        .debug_line - Line number information
                                        .debug_loc - Location lists used in the DW_AT_location attributes
                                        .debug_macinfo - Macro information
                                        .debug_frame - Call frame information
                                        .debug_ranges - Address ranges used in the DW_AT_ranges attributes
                                        .debug_str - String table used in .debug_info
                                        .debug_types - Type descriptions
                                        */
                                    _dwarfVersion = 2; //Dwarf version 2+
                                }
                            }
                        }
                    }

                    //Grab our symbol tables REF: http://docs.oracle.com/cd/E19620-01/805-4693/6j4emccrq/index.html
                    if (_symtabIdx > 0)
                    {
                        var symTabHdr = _sectionHeaders[_symtabIdx];
                        var symTabData = symTabHdr.SectionData;
                        var symTabStrTab = _sectionHeaders[symTabHdr.Link].SectionData;
                        _symtab = new ElfSectionSymbolTable(this, symTabData, symTabHdr, symTabStrTab);
                    }

                    if (_dynsymtabIdx > 0)
                    {
                        var symTabHdr = _sectionHeaders[_dynsymtabIdx];
                        var symTabData = symTabHdr.SectionData;
                        var symTabStrTab = _sectionHeaders[symTabHdr.Link].SectionData;
                        _dynsymtab = new ElfSectionSymbolTable(this, symTabData, symTabHdr, symTabStrTab);
                    }
                }
                else
                {
                    _sectionHeaders = new ElfSectionHeader[0];
                }

                StatusUpdateLog.OnReportProgress(Name, 1, "Creating Section to Segment Map...");
                //If both the segment and section headers are defined get section/segment mapping to resolve symbol addresses in the program versus file image
                if (_header._segmentHeaderOffset != 0 && _header._sectionHeaderOffset != 0)
                {
                    CreateSectionToSegmentMapping();
                }

                //Only do the following when someone requests it out of the current .elf file
                //GetCalibrationVersion();
                //GetBuildID();

                StatusUpdateLog.OnCloseProgress(Name);
            }
        }

        private void TryCombineSegmentHeaderInfo(ElfSegmentHeader matched, ElfSegmentHeader unmatched, ElfSegmentPair pair)
        {
            if (unmatched._sectionHeaders == null)
            {
                if (matched._offset == unmatched._offset &&
                   matched._memorySize == unmatched._memorySize)
                {
                    unmatched._sectionHeaders = matched._sectionHeaders;
                    if (matched._fileSize == unmatched._memorySize)
                    {
                        pair._fileSeg = matched;
                        pair._virtSeg = unmatched;
                        //Debug.WriteLine("\tPAIRED VIRT");
                    }
                    else if (unmatched._fileSize == matched._memorySize)
                    {
                        pair._fileSeg = unmatched;
                        pair._virtSeg = matched;
                        //Debug.WriteLine("\tPAIRED FILE");
                    }
                }
            }
        }

        private void CreateSectionToSegmentMapping()
        {
            UInt32 segIdx, secIdx;
            ElfSegmentHeader seg;
            ElfSectionHeader sec;

            UInt32 segIdxLen = (UInt32)_segmentHeaders.LongLength;
            UInt32 secIdxLen = (UInt32)_sectionHeaders.LongLength;

            StatusUpdateLog.OnStartProgress("Sec2Seg", segIdxLen + 3, Name);
            StatusUpdateLog.OnReportProgress("Sec2Seg", 0, "Sorting Sections...", Name);

            ElfSectionHeader[] sortedSectionHeaders = _sectionHeaders.OrderBy(h => h._offset).ToArray();
            ElfSegmentHeader[] sortedSegmentHeaders = _segmentHeaders.OrderBy(h => h._offset).ToArray();

            StatusUpdateLog.OnReportProgress("Sec2Seg", 1, "Mapping Segment 1...", Name);

            //_sectionSegmentMapping = new Dictionary<UInt32, ElfMapping>();
            //for (secIdx = 1; secIdx < secIdxLen; secIdx++)
            //    SectionSegmentMapping.Add(secIdx, new List<ElfSectionSegmentMapping>());

            //_segmentSectionMapping = new Dictionary<UInt32, List<ElfMapping>>();
            //for (segIdx = 0; segIdx < segIdxLen; segIdx++)
                //_segmentSectionMapping.Add(segIdx, new List<ElfMapping>());

            secIdx = 1;
            ElfSegmentHeader priorSeg = null;
            ElfSegmentPair currPair = null;
            for (segIdx = 0; segIdx < segIdxLen; segIdx++)
            {
                //get the next segment header
                seg = sortedSegmentHeaders[segIdx];
                //Debug.WriteLine(seg.ToString());

                //Check each section to see if it's contained by the segment
                //NOTE: Section can be contained by multiple segments however can only be in one physical and one memory location
                //skip null sections, sections that are not memory allocated
                for (; secIdx < secIdxLen; secIdx++)
                {
                    sec = sortedSectionHeaders[secIdx];
                    //Debug.WriteLine("\t" + sec.ToString());
                    //if (sec._index == 238)
                    //    Debug.WriteLine("here");
                    if ((sec._offset + sec._size) > (seg._offset + seg._memorySize))
                    {
                        //try and match the previous segment
                        if (currPair != null)
                        {
                            TryCombineSegmentHeaderInfo(currPair._virtSeg, seg, currPair);
                        }
                        break;
                    }
                    if (sec.SectionType == ElfSection.Type.NULL) continue;
                    if (!sec.IsMemoryAllocated) continue;
                    //if (sec._size == 0) continue;
                    if (sec._offset < seg._offset) continue;


                    //otherwise check for inclusion in the segment memory
                    if (sec._address >= seg._virtualAddress &&
                      ((sec._address + sec._size) <= (seg._virtualAddress + seg._memorySize)))
                    {
                        seg.AddSection(sec);
                        if (currPair == null || (currPair._virtSeg != seg && currPair._fileSeg != seg))
                        {
                            currPair = new ElfSegmentPair()
                            {
                                _fileSeg = seg,
                                _virtSeg = seg
                            };
                            //Debug.WriteLine("\tNEW SINGLE");

                            //Check the segment before and after to see if there is a file or virtual mapping
                            if (priorSeg != null)
                            {
                                TryCombineSegmentHeaderInfo(seg, priorSeg, currPair);
                                priorSeg = null;
                            }
                        }
                        var newMap = new ElfMapping2()
                        {
                            _sec = sec,
                            _segs = currPair
                        };
                        sec._segmentMapping = newMap;
                        //Debug.WriteLine("\tMAP:" + newMap.ToString());


                        if (seg._virtualAddress + seg._memorySize == sec._address + sec._size)
                        {
                            secIdx++;
                            break; //go to next segment
                        }
                    } else {
                        //Check to see if this matches previous segment
                        if(currPair!=null)
                        {
                            TryCombineSegmentHeaderInfo(currPair._virtSeg, seg, currPair);
                        }
                        //If it didn't match with the last seg... check if it matches the next
                        if (!seg.HasSections)
                        {
                            priorSeg = seg;
                            break;
                        }
                        else priorSeg = null;
                    }
                }
                StatusUpdateLog.OnReportProgress("Sec2Seg", 1, $"Mapping Segment [{segIdx}]...", Name);
            }

            StatusUpdateLog.OnReportProgress("Sec2Seg", 0, $"Second pass on segments...", Name);
            //do a second pass... check to see if each segment is satisfied...for each segment that doesn't have a section
            //associated... check the previous segment to see if the offset and memory size are the same if so... use the
            //new segments address as the memory address and the original as the physical address
            //NOTE: will only work if memory is at most copied once in this manner
            /*
            ElfSegmentHeader seg2;
            for (segIdx = 1; segIdx < segIdxLen; segIdx++)
            {
                //if (_segmentSectionMapping[segIdx].Count > 0) continue;

                seg = _segmentHeaders[segIdx];

                //check previous segment to see if it has the same offset and memory size
                if (seg._memorySize > 0) // only check segments of nonzero size
                {
                    seg2 = _segmentHeaders[segIdx - 1];
                    if (seg2._offset == seg._offset && seg2._memorySize == seg._memorySize)
                    {
                        if (seg._fileSize == 0) //seg contains virtual address of previous
                        {
                            //loop over each mapping and update the memory location
                            foreach (var mapping in _segmentSectionMapping[segIdx - 1])
                                mapping._memoryAddress = seg._virtualAddress + _sectionHeaders[mapping._sectionIdx]._address - seg2._virtualAddress;
                        }
                        else if (seg._fileSize > 0) //seg contains file address of previous
                        {
                            foreach (var mapping in _segmentSectionMapping[segIdx - 1])
                            {
                                mapping._fileAddress = (seg._physicalAddress != 0 ? seg._physicalAddress : seg._virtualAddress) + _sectionHeaders[mapping._sectionIdx]._address - seg2._virtualAddress;
                                mapping._fileSize = _sectionHeaders[mapping._sectionIdx]._size;
                            }
                        }

                    }
                }
            } */

            StatusUpdateLog.OnCloseProgress("Sec2Seg", Name);
        }

        public static bool TryGetFile(string path, out GTISFile file, VariableTable table = null, StatusUpdateDelegate statusHandlers = null)
        {
            //try
            //{
            string extension = Path.GetExtension(path).ToLower();
            if (VALID_EXTENSIONS.Contains(extension))
            {
                file = new ElfFile(path, statusHandlers);
                if (file.TrustLevel == TrustLevel.Error)
                    file = null;
                else if (table != null)
                    table.AddVariablesFromElf(file as ElfFile);
            }
            else file = null;
            //}
            //catch (Exception ex) { Debug.Print(ex.ToString()); file = null; }
            return file != null;
        }
    }
}
