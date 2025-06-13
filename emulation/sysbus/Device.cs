using System;

namespace EmbedEmul.SystemBus
{
    #region INTERFACE CONTRACT

    public enum Endianness
    {
        Unknown,
        Little,
        Big
    }
    /// <summary>
    /// Defines the contract for a device that can be connected to the bus.
    /// Devices interact with the bus through memory-mapped I/O operations.
    /// All addresses passed to IDevice methods are offsets from the device's base address.
    /// </summary>
    public interface IBusDevice
    {
        string Name { get; }
        UInt64 BusSize { get; }
        byte GetUInt8(UInt64 offset);
        void SetUInt8(UInt64 offset, byte value);
        UInt16 GetUInt16(UInt64 offset);
        void SetUInt16(UInt64 offset, UInt16 value);
        UInt32 GetUInt32(UInt64 offset);
        void SetUInt32(UInt64 offset, UInt32 value);
        UInt64 GetUInt64(UInt64 offset);
        void SetUInt64(UInt64 offset, UInt64 value);
    }
    #endregion

    #region MEMORY DEVICE
    public class BasicMemory : IBusDevice
    {
        Endianness _byteOrder = Endianness.Unknown;
        byte[] _data;

        private string _name;
        public string Name { get { return _name; } }
        public UInt64 BusSize { get { return (ulong)_data.Length; } }

        public BasicMemory(string name, UInt64 size)
        {
            _data = new byte[size];
            _name = name;
        }

        private bool AsLittleEndian()
        {
            return BitConverter.IsLittleEndian ^ (_byteOrder == Endianness.Little);
        }

        public byte GetUInt8(UInt64 offset)
        {
            return _data[offset];
        }

        public void SetUInt8(UInt64 offset, byte value)
        {
            _data[offset] = value;
        }

        public UInt16 GetUInt16(UInt64 offset)
        {
            UInt16 value;
            if (AsLittleEndian())
            {
                value = _data[offset];
                value |= (UInt16)(_data[offset + 1] << 8);
            }
            else
            {
                value = (UInt16)(_data[offset] << 8);
                value |= _data[offset + 1];
            }
            return value;
        }

        public void SetUInt16(UInt64 offset, UInt16 value)
        {
            if (AsLittleEndian())
            {
                _data[offset] = (byte)(value & 0xFF);
                _data[offset + 1] = (byte)((value >> 8) & 0xFF);
            }
            else
            {
                _data[offset] = (byte)((value >> 8) & 0xFF);
                _data[offset + 1] = (byte)(value & 0xFF);
            }
        }


        public UInt32 GetUInt32(UInt64 offset)
        {
            UInt32 value;
            if (AsLittleEndian())
            {
                value = _data[offset];
                value |= (UInt32)_data[offset + 1] << 8;
                value |= (UInt32)_data[offset + 2] << 16;
                value |= (UInt32)_data[offset + 3] << 24;
            }
            else
            {
                value = (UInt32)_data[offset] << 24;
                value |= (UInt32)_data[offset + 1] << 16;
                value |= (UInt32)_data[offset + 2] << 8;
                value |= _data[offset + 3];
            }
            return value;
        }

        public void SetUInt32(UInt64 offset, UInt32 value)
        {
            if (AsLittleEndian())
            {
                _data[offset] = (byte)(value & 0xFF);
                _data[offset + 1] = (byte)((value >> 8) & 0xFF);
                _data[offset + 2] = (byte)((value >> 16) & 0xFF);
                _data[offset + 3] = (byte)((value >> 24) & 0xFF);
            }
            else
            {
                _data[offset] = (byte)((value >> 24) & 0xFF);
                _data[offset + 1] = (byte)((value >> 16) & 0xFF);
                _data[offset + 2] = (byte)((value >> 8) & 0xFF);
                _data[offset + 3] = (byte)(value & 0xFF);
            }
        }

        public UInt64 GetUInt64(UInt64 offset)
        {
            UInt64 value;
            if (AsLittleEndian())
            {
                value = _data[offset];
                value |= (UInt64)_data[offset + 1] << 8;
                value |= (UInt64)_data[offset + 2] << 16;
                value |= (UInt64)_data[offset + 3] << 24;
                value |= (UInt64)_data[offset + 4] << 32;
                value |= (UInt64)_data[offset + 5] << 40;
                value |= (UInt64)_data[offset + 6] << 48;
                value |= (UInt64)_data[offset + 7] << 56;
            }
            else
            {
                value = (UInt64)_data[offset] << 56;
                value |= (UInt64)_data[offset + 1] << 48;
                value |= (UInt64)_data[offset + 2] << 40;
                value |= (UInt64)_data[offset + 3] << 32;
                value |= (UInt64)_data[offset + 4] << 24;
                value |= (UInt64)_data[offset + 5] << 16;
                value |= (UInt64)_data[offset + 6] << 8;
                value |= _data[offset + 7];
            }
            return value;
        }

        public void SetUInt64(UInt64 offset, UInt64 value)
        {
            if (AsLittleEndian())
            {
                _data[offset] = (byte)(value & 0xFF);
                _data[offset + 1] = (byte)((value >> 8) & 0xFF);
                _data[offset + 2] = (byte)((value >> 16) & 0xFF);
                _data[offset + 3] = (byte)((value >> 24) & 0xFF);
                _data[offset + 4] = (byte)((value >> 32) & 0xFF);
                _data[offset + 5] = (byte)((value >> 40) & 0xFF);
                _data[offset + 6] = (byte)((value >> 48) & 0xFF);
                _data[offset + 7] = (byte)((value >> 56) & 0xFF);
            }
            else
            {
                _data[offset] = (byte)((value >> 56) & 0xFF);
                _data[offset + 1] = (byte)((value >> 48) & 0xFF);
                _data[offset + 2] = (byte)((value >> 40) & 0xFF);
                _data[offset + 3] = (byte)((value >> 32) & 0xFF);
                _data[offset + 4] = (byte)((value >> 24) & 0xFF);
                _data[offset + 5] = (byte)((value >> 16) & 0xFF);
                _data[offset + 6] = (byte)((value >> 8) & 0xFF);
                _data[offset + 7] = (byte)(value & 0xFF);
            }
        }
    }

    #endregion

}
