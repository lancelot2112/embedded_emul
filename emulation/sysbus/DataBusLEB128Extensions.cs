using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region LEB128 EXTENSIONS
    public static class DataBusLEB128Extensions
    {
        public static UInt64 GetULEB128(this IDataBus bus)
        {
            UInt64 value;
            UInt32 pos = 0;
            byte val;
            value = 0;
            do
            {
                val = bus.GetUInt8();
                value |= (UInt64)((UInt64)(val & 0x7f) << (int)(7 * pos));
                pos++;

            } while ((pos < 8) && (val & 0x80) == 0x80);
            return value;
        }
        public static void SetULEB128(this IDataBus bus, UInt64 value)
        {
            throw new NotImplementedException("SetULEB128");
        }
        public static Int64 GetSLEB128(this IDataBus bus)
        {
            Int64 value;
            UInt32 pos = 0;
            byte val;
            int shift = 0;
            value = 0;
            do
            {
                val = bus.GetUInt8();
                value |= (val & 0x7f) << shift;
                shift += 7;
                pos++;
            } while ((val & 0x80) == 0x80);

            if (shift < 64 && (val & 0x40) == 0x40)
                value |= -(1 << shift);

            return value;
        }
        public static void SetSLEB128(this IDataBus bus, Int64 value)
        {
            throw new NotImplementedException("SetSLEB128");
        }
    }
    #endregion
}