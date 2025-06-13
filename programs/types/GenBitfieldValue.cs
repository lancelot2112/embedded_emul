using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;
using EmbedEmul.Tools;
using EmbedEmul.Variables;

namespace EmbedEmul.Types
{
    public class GenBitfieldValue : GenBaseValue
    {
        internal Int32 _bitOffset;
        internal Int32 _bitLength;

        public GenBitfieldValue(GenBaseValue baseValue, Int32 bitOffset, Int32 bitLength)
            :base(baseValue._name, baseValue._byteSize, baseValue._encoding)
        {
            _bitOffset = bitOffset;
            _bitLength = bitLength;
        }

        public override ValueCheckCode GetString(MemoryManager data, out string value, DisplayFormat displayFormat = DisplayFormat.Unknown)
        {
            return base.GetString(data, out value, displayFormat);
        }

        public override ValueCheckCode GetSigned(MemoryManager data, out Int64 value)
        {
            UInt64 fullValue;
            ValueCheckCode code = base.GetUnsigned(data, out fullValue);

            if (_bitLength < 64)
                value = (Int64)(fullValue & (((UInt64)1 << _bitLength) - 1)) >> _bitOffset;
            else
                value = (Int64)fullValue;

            return code;
        }

        public override ValueCheckCode GetUnsigned(MemoryManager data, out UInt64 value)
        {
            UInt64 fullValue;
            ValueCheckCode code = base.GetUnsigned(data, out fullValue);

            if (_bitLength < 64)
                value = (fullValue & (((UInt64)1 << _bitLength) - 1)) >> _bitOffset;
            else
                value = fullValue;

            return code;
        }

        public override ValueCheckCode GetFloat(MemoryManager data, out double value)
        {
            throw new NotImplementedException();
            //return base.GetFloat(data, address, out value);
        }

        public override ValueCheckCode SetString(string value, MemoryManager data, DisplayFormat format = DisplayFormat.Unknown)
        {
            throw new NotImplementedException();
            //return base.SetString(value, data, address, format);
        }

        public override ValueCheckCode SetSigned(long value, MemoryManager data)
        {
            throw new NotImplementedException();
            //return base.SetSigned(value, data, address);
        }

        public override ValueCheckCode SetUnsigned(ulong value, MemoryManager data)
        {
            throw new NotImplementedException();
            //return base.SetUnsigned(value, data, address);
        }

        public override ValueCheckCode SetFloat(double value, MemoryManager data)
        {
            throw new NotImplementedException();
            //return base.SetFloat(value, data, address);
        }
    }
}
