using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Elf
{
    public class ElfSegmentHeader
    {
        internal const int TOTAL_SIZE = 32;

        public ElfSegment.Type Type { get { return (ElfSegment.Type)_type; } }
        internal UInt32 _type;

        public UInt32 Offset { get { return _offset; } }
        internal UInt32 _offset;

        public UInt32 VirtualAddress { get { return _virtualAddress; } }
        internal UInt32 _virtualAddress;

        public UInt32 PhysicalAddress { get { return _physicalAddress; } }
        internal UInt32 _physicalAddress;

        public UInt32 FileSize { get { return _fileSize; } }
        internal UInt32 _fileSize;

        public UInt32 MemorySize { get { return _memorySize; } }
        internal UInt32 _memorySize;

        public UInt32 Flags { get { return _flags; } }
        internal UInt32 _flags;

        public bool IsReadable { get { return (_flags & 0x4) == 0x4; } }
        public bool IsWriteable { get { return (_flags & 0x2) == 0x2; } }
        public bool IsExecutable { get { return (_flags & 0x1) == 0x1; } }

        public UInt32 Alignment { get { return _align; } }
        internal UInt32 _align;

        internal UInt32 _index;


        public bool HasSections { get{ return _sectionHeaders != null && _sectionHeaders.Count > 0; } }
        public IEnumerable<ElfSectionHeader> SectionsContained { get{ if (_sectionHeaders == null) yield break; else foreach (var sec in _sectionHeaders) yield return sec; } }
        internal List<ElfSectionHeader> _sectionHeaders;
        public ElfSegmentHeader(System.IO.BinaryReader stream, ByteOrder order, UInt32 index)
            : this(new MemoryUnit(stream.ReadBytes(TOTAL_SIZE), order), index)
        { }

        public ElfSegmentHeader(MemoryUnit data, UInt32 index)
        {
            _type = data.GetUInt32();
            _offset = data.GetUInt32();
            _virtualAddress = data.GetUInt32();
            _physicalAddress = data.GetUInt32();
            _fileSize = data.GetUInt32();
            _memorySize = data.GetUInt32();
            _flags = data.GetUInt32();
            _align = data.GetUInt32();
            _index = index;
        }

        public void AddSection(ElfSectionHeader sec)
        {
            if(_sectionHeaders == null)
            {
                _sectionHeaders = new List<ElfSectionHeader>(1);
            }
            _sectionHeaders.Add(sec);
        }

        public string DisplayFlags()
        {
            StringBuilder build = new StringBuilder();
            build.AppendFormat("({0:X})", _flags);
            build.Append(IsReadable ? "R" : "-");
            build.Append(IsWriteable ? "W" : "-");
            build.Append(IsExecutable ? "X" : "-");
            return build.ToString();
        }

        public override string ToString()
        {
            const string entryFormat = "{0} off:{1:X} virt:{2:X} phys:{3:X} filesz:{4:X} memsz:{5:X} flg:{6} algn:{7:X}\n";
            return string.Format(entryFormat, Type, Offset, VirtualAddress, PhysicalAddress, FileSize,
                    MemorySize, DisplayFlags(), Alignment);
        }

        public string PrintMappedSections()
        {
            StringBuilder builder = new StringBuilder();
            builder.AppendLine("===Contained Sections===");

            foreach (var sec in SectionsContained)
                builder.AppendLine(sec.ToString());
            return builder.ToString();

        }


    }
}
