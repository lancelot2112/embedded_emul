using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region SIGNED EXTENSIONS
    public static class DataBusSignedExtensions
    {
        public static sbyte GetInt8(this IDataBus bus)
        {
            return (sbyte)bus.GetUInt8();
        }
        public static void SetInt8(this IDataBus bus, byte value)
        {
            bus.SetUInt8((byte)value);
        }
        public static Int16 GetInt16(this IDataBus bus)
        {
            return (Int16)bus.GetUInt16();
        }
        public static void SetInt16(this IDataBus bus, Int16 value)
        {
            bus.SetUInt16((UInt16)value);
        }
        public static Int32 GetInt32(this IDataBus bus)
        {
            return (Int32)bus.GetUInt32();
        }
        public static void SetInt32(this IDataBus bus, Int32 value)
        {
            bus.SetUInt32((UInt32)value);
        }
        public static Int64 GetInt64(this IDataBus bus)
        {
            return (Int64)bus.GetUInt64();
        }
        public static void SetInt64(this IDataBus bus, Int64 value)
        {
            bus.SetUInt64((UInt64)value);
        }
    }
    #endregion
}