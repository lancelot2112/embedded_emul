using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Elf
{
    public class ElfSectionHeader
    {
        //Calculated by adding number of bytes in each member
        const int TOTAL_LEN = 40;

        /// <summary>
        /// Index into section header string table section defining section name.
        /// </summary>
        internal UInt32 _nameIndex;

        /// <summary>
        /// Section name.
        /// </summary>
        public string Name { get { return _name; } }
        internal string _name;

        /// <summary>
        /// Defines the type of section which leads to different data interpretations later on.
        /// </summary>
        public ElfSection.Type SectionType { get { return (ElfSection.Type)_type; } }
        internal UInt32 _type;

        /// <summary>
        /// Array of 32 1 bit flags
        /// </summary>
        public UInt32 Flags { get { return _flags; } }
        internal UInt32 _flags;

        public bool IsWriteable { get { return (_flags & 0x1) == 0x1; } }
        public bool IsMemoryAllocated { get { return (_flags & 0x2) == 0x2; } }
        public bool IsExecutable { get { return (_flags & 0x4) == 0x4; } }

        /// <summary>
        /// Gives address of first byte inside the memory image of a process.
        /// </summary>
        public UInt32 Address { get { return _address; } }
        internal UInt32 _address;

        /// <summary>
        /// Gives the section offset from the start of file in bytes.
        /// </summary>
        public UInt32 Offset { get { return _offset; } }
        internal UInt32 _offset;

        /// <summary>
        /// Gives the section size in bytes.  SHT_NOBITS may have a nonzero value here
        /// but will occupy no actual length of file.
        /// </summary>
        public UInt32 Size { get { return _size; } }
        internal UInt32 _size;

        /// <summary>
        /// Section header table index link, whose interpretation depends on section type.
        /// </summary>
        public UInt32 Link { get { return _link; } }
        internal UInt32 _link;

        /// <summary>
        /// Extra information interpreted per section type.
        /// </summary>
        public UInt32 Info { get { return _info; } }
        internal UInt32 _info;

        /// <summary>
        /// Address alignment constraint specifying a valid address must satisfy
        /// address % alignment == 0
        /// </summary>
        public UInt32 Alignment { get { return _addressAlign; } }
        internal UInt32 _addressAlign;

        /// <summary>
        /// Some section types have a table of fixed-size entries, such as a symbol table.
        /// This value is the size of each table entry in bytes.
        /// </summary>
        public UInt32 EntitySize { get { return _entitySize; } }
        internal UInt32 _entitySize;

        public UInt32 Index { get { return _index; } }
        internal UInt32 _index;

        public ElfMapping2 SegmentMapping { get { return _segmentMapping; } }
        internal ElfMapping2 _segmentMapping;

        internal MemoryUnit _data;
        public MemoryUnit SectionData { get { return _data; } internal set { _data = value; } }

        public ElfSectionHeader(System.IO.BinaryReader stream, ByteOrder order, UInt32 index)
            : this(new MemoryUnit(stream.ReadBytes(TOTAL_LEN), order), index)
        { }

        public ElfSectionHeader(MemoryUnit data, UInt32 index)
        {
            _nameIndex = data.GetUInt32();
            _type = data.GetUInt32();
            _flags = data.GetUInt32();
            _address = data.GetUInt32();
            _offset = data.GetUInt32();
            _size = data.GetUInt32();
            _link = data.GetUInt32();
            _info = data.GetUInt32();
            _addressAlign = data.GetUInt32();
            _entitySize = data.GetUInt32();
            _index = index;
        }

        public string DisplayFlags()
        {
            StringBuilder build = new StringBuilder();
            build.AppendFormat("({0:X})", _flags);
            build.Append(IsMemoryAllocated ? "A" : "-");
            build.Append(IsWriteable ? "W" : "-");
            build.Append(IsExecutable ? "X" : "-");
            return build.ToString();
        }

        public override string ToString()
        {
            const string entryFormat = "{0} {1} addr:{2:X} off:{3:X} sz:{4:X} entsz:{5:X} flg:{6} lnk:{7:X} inf:{8:X} algn:{9}";

            return string.Format(entryFormat, Name, SectionType, Address, Offset, Size, EntitySize,
                    DisplayFlags(), Link, Info, Alignment); ;
        }
    }
}
