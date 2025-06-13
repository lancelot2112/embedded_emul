using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.SystemBus
{
    #region INTERFACE CONTRACT

    public enum BusStatus
    {
        AddressValid,
        AddressNotMapped
    }

    public interface IDeviceBus
    {
        bool ResolvesToRange(UInt64 busAddress, out IBusRange range);
        /// <summary>
        /// Registers a device on the bus within a specified address range.
        /// </summary>
        /// <param name="device">The device to register.</param>
        /// <param name="busAddress">The starting global address where the device will be mapped.</param>
        /// <exception cref="ArgumentNullException">Thrown if device is null.</exception>
        /// <exception cref="ArgumentOutOfRangeException">Thrown if size is 0.</exception>
        /// <exception cref="BusOperationException">Thrown if the address range conflicts with an existing device or redirection.</exception>
        void RegisterDevice(IBusDevice device, UInt64 busAddress);
        /// <summary>
        /// Unregisters a device from the bus.
        /// </summary>
        /// <param name="device">The device to unregister.</param>
        /// <returns>True if the device was found and unregistered, false otherwise.</returns>
        bool UnregisterDevice(IBusDevice device);

        bool TryGetDevice(string name, out DeviceRegistration device);
        bool TryGetDevice(int index, out DeviceRegistration device);

        /// <summary>
        /// Unregisters a device mapped at the specified start address.
        /// </summary>
        /// <param name="startAddress">The starting global address of the device to unregister.</param>
        /// <returns>The device that was unregistered, or null if no device was mapped at that start address.</returns>
        IBusDevice UnregisterDeviceAt(UInt64 startAddress);

        /// Adds an address redirection rule to the bus.
        /// Accesses to the source address range will be transparently redirected to the target address range.
        /// Redirections are processed before device lookups.
        /// </summary>
        /// <param name="sourceStartAddress">The start of the source address range to redirect.</param>
        /// <param name="size">The size of the address range to redirect. Must be greater than 0.</param>
        /// <param name="targetStartAddress">The start of the target address range.</param>
        /// <remarks>
        /// For example, if sourceStartAddress=0x1000, size=0x100, targetStartAddress=0x8000,
        /// then an access to 0x1050 will be redirected to 0x8050.
        /// Redirection chains or conflicts should be handled by the implementation (e.g., by disallowing them or defining precedence).
        /// </remarks>
        /// <exception cref="ArgumentOutOfRangeException">Thrown if size is 0.</exception>
        /// <exception cref="BusOperationException">Thrown if the redirection rule conflicts with existing mappings or is invalid.</exception>
        void Redirect(UInt64 sourceStartAddress, UInt64 size, UInt64 targetStartAddress);

        /// <summary>
        /// Removes an address redirection rule from the bus.
        /// </summary>
        /// <param name="sourceStartAddress">The start of the source address range of the redirection to remove.</param>
        /// <param name="size">The size of the address range of the redirection to remove.</param>
        /// <returns>True if the redirection rule was found and removed, false otherwise.</returns>
        bool RemoveRedirect(UInt64 sourceStartAddress, UInt64 size);
    }

    public record DeviceRegistration(IBusDevice Device,UInt64 Address);
    #endregion

    #region EXCEPTIONS
    /// <summary>
    /// Represents an exception that occurs during a bus operation.
    /// </summary>
    public class BusOperationException : Exception
    {
        public BusOperationException() { }
        public BusOperationException(string message) : base(message) { }
        public BusOperationException(string message, Exception inner) : base(message, inner) { }
    }

    /// <summary>
    /// Represents an exception for errors related to address mapping or access.
    /// </summary>
    public class BusAddressException : BusOperationException
    {
        public ulong Address { get; }

        public BusAddressException(ulong address, string message) : base(message)
        {
            Address = address;
        }

        public BusAddressException(ulong address, string message, Exception inner) : base(message, inner)
        {
            Address = address;
        }
    }
    #endregion

    #region BUS IMPLEMENTATION
    public class BasicHashedDeviceBus : IDeviceBus
    {
        Dictionary<UInt64, List<IBusRange>> _ranges = new Dictionary<UInt64, List<IBusRange>>();
        Dictionary<string, DeviceRegistration> _registrations = new Dictionary<string, DeviceRegistration>();
        List<DeviceRegistration> _registrationList = new List<DeviceRegistration>();
        /// <summary>
        /// Number of bits from the LSB to use as the Level2 address.  The
        /// level 1 address "hash" is calculated from this number as the following.
        /// ~((1<<(AddressBits))-1)
        /// eg. If AddressBits is 20 (and address size is 32) then the hash mask will be 0xFFF00000
        /// </summary>
        private byte _addrBits;
        private UInt64 _hashMask;

        public BasicHashedDeviceBus(byte addrSize = 32, byte hashBits = 12)
        {
            _addrBits = (byte)(addrSize - hashBits);
            _hashMask = (UInt64)~((1 << _addrBits) - 1);
        }


        private UInt64 GetHash(UInt64 address)
        {
            return (UInt64)(address & _hashMask) >> _addrBits;
        }

        private UInt64 GetAddress(UInt64 hash)
        {
            return (UInt64)(hash << _addrBits);
        }

        /// <summary>
        /// Finds the range containing the address and calculates
        /// the index offset into the returned memory unit for later access
        /// </summary>
        /// <param name="address">Input address to seek, output the index from the start of the memory range</param>
        /// <param name="unit">The current memory unit</param>
        /// <returns>True if found, false if not found</returns>
        public bool ResolvesToRange(UInt64 address, out IBusRange busRange)
        {
            UInt64 hash = GetHash(address);

            busRange = null;
            List<IBusRange> ranges;
            if (_ranges.TryGetValue(hash, out ranges))
            {
                int idx = 0;

                //Find a redirect in our range
                while (idx < ranges.Count && ranges[idx].BusEnd <= address) idx++;
                if (idx < ranges.Count && ranges[idx].BusStart <= address)
                {
                    busRange = ranges[idx];
                    return true;
                }
                else return false;
            }
            else return false;
        }

        private void SplitAndInsert(IBusRange rangeToInsert)
        {
            UInt64 idx = GetHash(rangeToInsert.BusStart);
            UInt64 endIdx = GetHash(rangeToInsert.BusEnd);
            if (idx == endIdx) //No splitting necessary
            {
                InsertRange(rangeToInsert);
            }
            else
            {
                UInt64 hashStart;
                UInt64 hashEnd;

                for (; idx < endIdx; idx++)
                {
                    hashStart = GetAddress(idx);
                    hashEnd = GetAddress(idx + 1);
                    if (hashStart > rangeToInsert.BusStart && hashEnd > rangeToInsert.BusEnd)
                    {
                        if (hashEnd > rangeToInsert.BusEnd)
                        {

                        }
                        else
                        {

                        }
                    }
                    else if (hashStart <= rangeToInsert.BusStart)
                    {

                    }
                }
            }
        }
        private void InsertRange(IBusRange rangeToInsert)
        {
            UInt64 idx = GetHash(rangeToInsert.BusStart);
            UInt64 endIdx = GetHash(rangeToInsert.BusEnd);
            if (idx != endIdx)
                throw new BusOperationException("INTERNAL BUS ERROR: Inserted range spans mutiple hash indices, split range before passing to InsertRange.");

            List<IBusRange> ranges;
            if (_ranges.TryGetValue(idx, out ranges))
            {
                Int32 l2Idx = 0;
                while (l2Idx < ranges.Count && ranges[l2Idx].BusStart < rangeToInsert.BusStart) l2Idx++;

                //Check for overlap
                if (l2Idx < ranges.Count && ranges[l2Idx].BusStart < rangeToInsert.BusEnd)
                {
                    throw new BusAddressException(rangeToInsert.BusStart, $"Cannot add or redirect device {rangeToInsert.Device.Name} overlaps with existing range for {ranges[l2Idx].Device.Name}");
                }

                //Insert sort - Shift everybody forward by one
                IBusRange next;
                IBusRange current = rangeToInsert;
                while (l2Idx < ranges.Count)
                {
                    next = ranges[l2Idx];
                    ranges[l2Idx++] = current;
                    current = next;
                }
                ranges.Add(current);
            }
            else
            {
                ranges = new List<IBusRange>() { rangeToInsert };
                _ranges.Add(idx, ranges);
            }
        }

        public bool TryGetDevice(string name, out DeviceRegistration reg)
        {
            return _registrations.TryGetValue(name, out reg);
        }

        public bool TryGetDevice(int index, out DeviceRegistration reg)
        {
            reg = null;
            if (_registrationList.Count > index)
                reg = _registrationList[index];

            return reg != null;
        }
        public void RegisterDevice(IBusDevice device, UInt64 busAddress)
        {
            IBusRange resolvedRange;
            if (ResolvesToRange(busAddress, out resolvedRange))
            {
                throw new BusAddressException(busAddress, $"Cannot register device {device.Name} overlaps with existing range for {resolvedRange.Device.Name}");
            }

            if (device.BusSize == 0)
            {
                throw new ArgumentOutOfRangeException(nameof(device.BusSize), $"Device {device.Name} size must be greater than 0 for registration.");
            }

            if (device == null)
            {
                throw new ArgumentNullException(nameof(device), "Device cannot be null.");
            }

            var registration = new DeviceRegistration(device, busAddress);
            _registrations.Add(device.Name, registration);
            _registrationList.Add(registration);

            var busRange = new BusRange(device, busAddress, device.BusSize);
            InsertRange(busRange);
        }

        public bool UnregisterDevice(IBusDevice unit)
        {
            return false;
        }

        public IBusDevice UnregisterDeviceAt(UInt64 busAddress)
        {
            return null;
        }

        public void Redirect(UInt64 sourceAddress, UInt64 size, UInt64 targetAddress)
        {
            IBusRange range;
            UInt64 deviceOffsetStart;
            if (!ResolvesToRange(targetAddress, out range))
            {
                throw new BusAddressException(sourceAddress, $"Cannot redirect {sourceAddress} to address {targetAddress} as it does not resolve to a registered device.");
            }

            if (size == 0)
            {
                throw new ArgumentOutOfRangeException(nameof(size), "Redirection size must be greater than 0.");
            }

            if (!range.Resolves(targetAddress, out deviceOffsetStart) || (deviceOffsetStart + size) > range.Size)
            {
                throw new BusAddressException(sourceAddress, $"Cannot redirect {sourceAddress} to address {targetAddress} with size {size} as it is not fully encapsulated in the device range.");
            }

            var busRedirect = new BusRange(range.Device, sourceAddress, size, deviceOffsetStart);
            InsertRange(busRedirect);
        }

        public bool RemoveRedirect(UInt64 sourceStartAddress, UInt64 size)
        {
            return false;
        }
    }
    #endregion
}