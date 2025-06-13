using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Elf
{
    public class ElfSegment
    {
        internal ElfHeader _context;
        public ElfHeader Context { get { return _context; } }
        internal ElfSegmentHeader _header;
        public ElfSegmentHeader Header { get { return _header; } }
        internal byte[] _data;
        public byte[] Data { get { return _data; } }

        public ElfSegment(ElfHeader context, ElfSegmentHeader header)
        {
            _context = context;
            _header = header;
        }

        public ElfSegment(ElfHeader context, ElfSegmentHeader header, System.IO.BinaryReader reader)
        {
            _context = context;
            _header = header;
            _data = GetData(reader);
        }

        public byte[] GetData(System.IO.BinaryReader reader)
        {
            reader.BaseStream.Seek(_context._elfHeaderOffset + _header._offset, System.IO.SeekOrigin.Begin);
            return reader.ReadBytes((int)_header._fileSize);
        }

        public enum Type
        {
            Null = 0,
            Load = 1,
            Dynamic = 2,
            Interpreter = 3,
            Note = 4,
            ShLib = 5,
            ProgramHeader = 6
            //0x70000000-0x7fffffff Process reserved
        }
    }
}
