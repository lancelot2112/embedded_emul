using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region FLOAT EXTENSIONS
    public static class DataBusFloatExtensions
    {
        #region SINGLE
        [StructLayout(LayoutKind.Explicit)]
        public struct UInt32FloatUnion
        {
            [FieldOffset(0)]
            public float f;
            [FieldOffset(0)]
            public UInt32 u;

            public UInt32FloatUnion(float value)
            {
                u = 0;
                f = value;
            }

            public UInt32FloatUnion(UInt32 value)
            {
                f = 0;
                u = value;
            }
        }

        public static float GetSingle(this IDataBus bus)
        {
            var reg = new UInt32FloatUnion(bus.GetUInt32());
            return reg.f;
        }
        public static void SetSingle(this IDataBus bus, float value)
        {
            var reg = new UInt32FloatUnion(value);
            bus.SetUInt32(reg.u);
        }
        #endregion

        #region DOUBLE
        [StructLayout(LayoutKind.Explicit)]
        public struct UInt64DoubleUnion
        {
            [FieldOffset(0)]
            public double d;
            [FieldOffset(0)]
            public UInt64 u;

            public UInt64DoubleUnion(double value)
            {
                u = 0;
                d = value;
            }

            public UInt64DoubleUnion(UInt64 value)
            {
                d = 0;
                u = value;
            }
        }

        public static double GetDouble(this IDataBus bus)
        {
            var reg = new UInt64DoubleUnion(bus.GetUInt64());
            return reg.d;
        }
        public static void SetDouble(this IDataBus bus, double value)
        {
            var reg = new UInt64DoubleUnion(value);
            bus.SetUInt64(reg.u);
        }
        #endregion
    }
    #endregion
}