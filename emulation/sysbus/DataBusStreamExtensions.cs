using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region IMPLEMENTATION
    public static class DataBusStreamExtension
    {
        public static IEnumerable<byte> GetBytes(this IDataBus bus, Int64 count)
        {
            while (count > 0 && bus.Available(1))
            {
                yield return bus.GetUInt8();
                count--;
            }
        }

        public static void SetBytes(this IDataBus bus, IEnumerable<byte> bytes)
        {
            foreach (byte value in bytes)
                bus.SetUInt8(value);
        }

        public static BusByteStream GetByteStream(this IDataBus bus, Int64 readLength = -1)
        {
            return new BusByteStream(bus, readLength);
        }
    }
    #endregion

    #region BYTE STREAM
    public class BusByteStream : Stream, IDisposable
    {
        private readonly IDataBus _bus;
        private bool _disposed;

        public BusByteStream(IDataBus bus, Int64 length = -1)
        {
            if (bus == null)
                throw new ArgumentNullException(nameof(bus));
            _bus = bus;

            var busRem = bus.BytesToEnd();
            if (length == -1 || busRem < (UInt64)length)
                _length = (Int64)busRem;
            else
                _length = length;
        }

        public override bool CanRead => true;
        public override bool CanSeek => false;
        public override bool CanWrite => false;
        long _length;
        public override long Length => _length;
        public override long Position
        {
            get { return (long)(_bus.DeviceOffset - _bus.JumpDeviceOffset); }
            set { _bus.JumpRelative(value); }
        }

        public override int Read(byte[] buffer, int offset, int count)
        {
            int i = 0;
            for (; i < count && _bus.Available(1) && Position < _length; i++)
                buffer[i + offset] = _bus.GetUInt8();
            return i;
        }

        public override long Seek(long offset, SeekOrigin origin) => throw new InvalidOperationException();
        public override void SetLength(long value) => throw new InvalidOperationException();
        public override void Write(byte[] buffer, int offset, int count) => throw new InvalidOperationException();
        public override void Flush() => throw new InvalidOperationException();

        void IDisposable.Dispose()
        {
            if (_disposed)
                return;
            _disposed = true;
            //_input.Dispose();
        }
    }
    #endregion
}