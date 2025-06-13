using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Security.Cryptography;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region CRYPTO EXTENSIONS
    public static class DataBusArbSizeExtensions
    {
        public static UInt64 GetValue(this IDataBus bus, byte size)
        {
            if (size == 1) return bus.GetUInt8();
            else if (size == 2) return bus.GetUInt16();
            else if (size == 4) return bus.GetUInt32();
            else if (size == 8) return bus.GetUInt64();
            else throw new NotImplementedException();
        }

        public static void SetValue(this IDataBus bus, byte size, UInt64 value)
        {
            if (size == 1) bus.SetUInt8((byte)value);
            else if (size == 2) bus.SetUInt16((ushort)value);
            else if (size == 4) bus.SetUInt32((uint)value);
            else if (size == 8) bus.SetUInt64(value);
            else throw new NotImplementedException();
        }
    }
    #endregion
}