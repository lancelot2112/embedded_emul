using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Elf
{
    public class ElfSection
    {
        internal ElfHeader _context;
        public ElfHeader Context { get { return _context; } }
        internal ElfSectionHeader _header;
        public ElfSectionHeader Header { get { return _header; } }
        internal MemoryUnit _data;
        public MemoryUnit Data { get { return _data; } }

        public ElfSection(ElfHeader context, ElfSectionHeader header, MemoryUnit data)
        {
            _context = context;
            _header = header;
            _data = data;
        }

        public ElfSection(ElfHeader context, ElfSectionHeader header, System.IO.BinaryReader reader)
        {
            _context = context;
            _header = header;
            _data = GetData(reader, context.ByteOrder);
        }

        public MemoryUnit GetData(System.IO.BinaryReader reader, ByteOrder order)
        {
            if (_header._type != (uint)Type.NOBITS)
            {
                reader.BaseStream.Seek(_context._elfHeaderOffset + _header._offset, System.IO.SeekOrigin.Begin);
                return new MemoryUnit(reader.ReadBytes((int)_header._size), order);
            }
            else
                return null;
        }

        public override string ToString()
        {
            return string.Format("{0} {1}", _header._name, _header.SectionType);
        }

        public enum Type : uint
        {
            NULL = 0,
            PROGBITS = 1,
            SYMTAB = 2,
            STRTAB = 3,
            REL = 4,
            HASH = 5,
            DYN = 6,
            NOTE = 7,
            NOBITS = 8,
            RELA = 9,
            SHLIB = 10,
            DYNSYMTAB = 11
            //0x70000000-0x7fffffff Process defined
            //0x80000000-0xffffffff User defined
        }
    }
}
