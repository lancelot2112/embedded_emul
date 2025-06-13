using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.SystemBus
{
    #region INTERFACE CONTRACT

    public interface IAddressBus
    {
        UInt64 BusAddress { get; set; }
        UInt64 DeviceOffset { get; }
        UInt64 JumpAddress { get; }
        UInt64 JumpDeviceOffset { get; }

        //Jumps to an absolute bus address
        bool Jump(UInt64 busAddress);
        //Jumps to a relative offset from the previous jumped address
        bool JumpRelative(Int64 offset);
        bool IncrOffset(UInt64 incr = 1);
        bool DecrOffset(UInt64 decr = 1);
        UInt64 BytesToEnd();
    }
    #endregion

    #region IMPLEMENTATION
    public class BasicBusAccess : IAddressBus
    {
        internal BusStatus _status;
        internal IDeviceBus _deviceBus;
        internal IBusRange _range;

        internal UInt64 _deviceOffset;
        internal UInt64 _deviceOffsetEnd;
        internal UInt64 _jumpDeviceOffset;
        internal UInt64 _jumpRegister;

        public UInt64 DeviceOffset
        {
            get { return _deviceOffset; }
        }

        public UInt64 JumpAddress
        {
            get { return _jumpRegister; }
        }

        public UInt64 JumpDeviceOffset
        {
            get { return _jumpDeviceOffset; }
        }
        public UInt64 BusAddress
        {
            get { return _jumpRegister + (_deviceOffset - _jumpDeviceOffset); }
            set { Jump(value); }
        }
        public BasicBusAccess(IDeviceBus deviceBus)
        {
            _deviceBus = deviceBus;
            _range = null;
        }

        public bool IncrOffset(UInt64 incr)
        {
            if (_deviceOffset + incr < _deviceOffsetEnd)
            {
                _deviceOffset += incr;
                return true;
            }

            throw new NotImplementedException();
        }

        public bool DecrOffset(UInt64 decr)
        {
            if (_deviceOffset >= decr)
            {
                _deviceOffset -= decr;
                return true;
            }

            throw new NotImplementedException();
        }

        public UInt64 BytesToEnd()
        {
            if (_deviceOffset >= _deviceOffsetEnd) return 0;

            return _deviceOffsetEnd - _deviceOffset;
        }

        public bool JumpRelative(Int64 offset)
        {
            Int64 jumpDest = (Int64)_jumpDeviceOffset + offset;
            if (jumpDest < 0 || (ulong)jumpDest >= _deviceOffsetEnd)
                return false;
            _deviceOffset = (UInt64)jumpDest;
            return true;

        }
        public bool Jump(UInt64 busAddress)
        {
            if (_range != null && _range.Resolves(busAddress, out _deviceOffset))
                return true;

            _deviceOffsetEnd = 0;
            if (!_deviceBus.ResolvesToRange(busAddress, out _range))
            {
                _status = BusStatus.AddressNotMapped;
                _range = null;
                return false;
                //throw new InvalidOperationException("No valid memory unit found. Please seek a valid address first.");
            }
            _status = BusStatus.AddressValid;
            _jumpRegister = busAddress;
            if (!_range.Resolves(_jumpRegister, out _jumpDeviceOffset))
                throw new InvalidOperationException("Resolved address didn't resolve in the range, should never get here due to upstream checks.");

            _deviceOffset = _jumpDeviceOffset;
            _deviceOffsetEnd = _range.Device.BusSize;
            return true;
        }
    }
    #endregion
}