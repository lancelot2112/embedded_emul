using System;
using System.Diagnostics;

namespace EmbedEmul.SystemBus
{
    #region INTERFACE CONTRACT
    public interface IBusRange
    {
        UInt64 StartDeviceOffset { get; }
        UInt64 BusStart { get; }
        UInt64 Size { get; }
        UInt64 BusEnd { get; }
        byte Priority { get; }
        IBusDevice Device { get; }
        bool Resolves(UInt64 busAddress, out UInt64 deviceOffset)
        {
            if (busAddress >= BusStart && busAddress <= BusEnd)
            {
                deviceOffset = StartDeviceOffset + busAddress - BusStart;
                return true;
            }

            deviceOffset = 0;
            return false;
        }
    }
    #endregion

    #region RANGE IMPLEMENTATION

    public class BusRange : IBusRange
    {
        internal UInt64 _startOffset;
        public UInt64 StartDeviceOffset => _startOffset;
        internal UInt64 _start;
        public UInt64 BusStart => _start;
        internal UInt64 _size;
        public UInt64 Size => _size;
        public UInt64 BusEnd => _start + _size;
        internal IBusDevice _device;
        public IBusDevice Device => _device;

        public byte Priority => 0;

        /// <summary>
        ///
        /// </summary>
        /// <param name="device"></param>
        /// <param name="start">Start of the address range</param>
        /// <param name="size">Length of the  range</param>
        /// <param name="offset">Start offset in the device range (0 for start of range)</param>
        public BusRange(IBusDevice device, UInt64 start, UInt64 size, UInt64 offset = 0)
        {
            //Debug.Assert(offset + size <= device.BusSize);
            _device = device;
            _start = start;
            _size = size;
            _startOffset = offset;
        }
    }
    #endregion
}