using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.SystemBus
{
    #region INTERFACE CONTRACT

    public interface IDataBus : IAddressBus
    {
        bool Available(byte size);

        byte GetUInt8();
        void SetUInt8(byte value);
        UInt16 GetUInt16();
        void SetUInt16(UInt16 value);
        UInt32 GetUInt32();
        void SetUInt32(UInt32 value);
        UInt64 GetUInt64();
        void SetUInt64(UInt64 value);
    }
    #endregion

    #region IMPLEMENTATION
    /// <summary>
    /// Keeps a resolved address and index and allows reads of various sizes
    /// </summary>
    public class BasicDataBus : BasicBusAccess, IDataBus
    {
        public BasicDataBus(IDeviceBus bus)
           : base(bus)
        { }
        public bool Available(byte count)
        {
            return BytesToEnd() >= count;
        }

        public byte GetUInt8()
        {
            byte value = _range.Device.GetUInt8(_deviceOffset);
            IncrOffset(1);
            return value;
        }

        public void SetUInt8(byte value)
        {
            _range.Device.SetUInt8(_deviceOffset, value);
            IncrOffset(1);
        }

        public UInt16 GetUInt16()
        {
            UInt16 value = _range.Device.GetUInt16(_deviceOffset);
            IncrOffset(2);
            return value;
        }

        public void SetUInt16(UInt16 value)
        {
            _range.Device.SetUInt16(_deviceOffset, value);
            IncrOffset(2);
        }
        public UInt32 GetUInt32()
        {
            UInt32 value = _range.Device.GetUInt32(_deviceOffset);
            IncrOffset(4);
            return value;
        }

        public void SetUInt32(UInt32 value)
        {
            _range.Device.SetUInt32(_deviceOffset, value);
            IncrOffset(4);
        }

        public UInt64 GetUInt64()
        {
            UInt64 value = _range.Device.GetUInt64(_deviceOffset);
            IncrOffset(8);
            return value;
        }

        public void SetUInt64(UInt64 value)
        {
            _range.Device.SetUInt64(_deviceOffset, value);
            IncrOffset(8);
        }
    }

    #endregion
}