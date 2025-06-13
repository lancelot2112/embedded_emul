using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public class DWTypeUnitHeader
    {
        internal byte _size;
        internal UInt32 _length;
        internal UInt16 _version;
        internal UInt32 _abbrevOffset;
        internal byte _addressSize;
        internal UInt64 _typeSignature;
        internal UInt32 _typeOffset;

        public DWTypeUnitHeader(MemoryUnit data)
        {
            _length = data.GetUInt32();
            if (_length <= 0xfffffff0) _size = 23;
            else throw new NotSupportedException("64bit DWARF not supported, TypeUnitHeader.");
            _version = data.GetUInt16();
            _abbrevOffset = data.GetUInt32();
            _addressSize = data.GetUInt8();
            _typeSignature = data.GetUInt64();
            _typeOffset = data.GetUInt32();
        }
    }
}
