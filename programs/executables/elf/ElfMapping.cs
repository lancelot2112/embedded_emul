using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Elf
{
    public class ElfMapping
    {
        internal UInt32 _segmentIdx;
        public UInt32 SegmentIndex { get { return _segmentIdx; } }
        internal UInt32 _sectionIdx;
        public UInt32 SectionIndex { get { return _sectionIdx; } }
        internal UInt32 _memoryAddress;
        public UInt32 MemoryAddress { get { return _memoryAddress; } }
        internal UInt32 _memorySize;
        public UInt32 MemorySize { get { return _memorySize; } }
        internal UInt32 _fileAddress;
        public UInt32 FileAddress { get { return _fileAddress; } }
        internal UInt32 _fileSize;
        public UInt32 FileSize { get { return _fileSize; } }

        public override string ToString()
        {
            return string.Format("{0:X} {1:X} {2:X}_{3:X} {4:X}_{5:X}",
                _segmentIdx, _sectionIdx, _memoryAddress, _memorySize, _fileAddress, _fileSize);
        }
    }

    public class ElfSegmentPair
    {
        internal ElfSegmentHeader _fileSeg;
        internal ElfSegmentHeader _virtSeg;
    }
    public class ElfMapping2
    {
        internal ElfSegmentPair _segs;
        public ElfSegmentHeader MemorySegment { get{ return _segs._virtSeg; } }
        public ElfSegmentHeader FileSegment { get{ return _segs._fileSeg; } }
        internal ElfSectionHeader _sec;

        private bool _secAddrIsFileAddr
        {
            get 
            {
                UInt32 segFileAddress = _segFileAddress;
                return _sec._address >= segFileAddress && ((_sec._address + _sec._size) <= (segFileAddress + _segs._fileSeg._fileSize)); 
            }
        }
        private UInt32 _segFileAddress
        {
            get { return (_segs._fileSeg._physicalAddress != 0) ? _segs._fileSeg._physicalAddress : _segs._fileSeg._virtualAddress; }
        }

        public UInt32 MemoryAddress
        {
            get
            {
                if (!_secAddrIsFileAddr)
                {
                    return _sec._address;
                }
                else
                {
                    UInt32 segFileAddr = _segFileAddress;
                    UInt32 segVirtAddr = _segs._virtSeg._virtualAddress;
                    return segVirtAddr + _sec._address - segFileAddr;
                }

            }
        }
        public UInt32 MemorySize { get { return _sec._size; } }
        public UInt32 FileAddress
        {
            get
            {
                if (FileSize == 0) return 0;


                if (_secAddrIsFileAddr)
                {
                    return _sec._address;
                }
                else
                {
                    UInt32 segFileAddr = _segFileAddress;
                    UInt32 segVirtAddr = _segs._virtSeg._virtualAddress;
                    return segFileAddr + _sec._address - segVirtAddr;
                }
            }
        }
        public UInt32 FileSize { get { return (_segs._fileSeg._fileSize != 0) ? _sec._size : 0; } }

        public override string ToString()
        {
            return $"{_sec._name} virt:{_segs._virtSeg._index} {MemoryAddress:X}_{MemorySize:X} file:{_segs._fileSeg._index} {FileAddress:X}_{FileSize:X}";
        }
    }
}
