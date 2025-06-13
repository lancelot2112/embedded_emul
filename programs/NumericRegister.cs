using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul
{
    //Define a union type to be able to convert to floating point value
    //after assigning the raw bytes to the UInt64 value
    [StructLayout(LayoutKind.Explicit)]
    public struct NumericRegister
    {
        public static NumericRegister Zero = new NumericRegister();

        [FieldOffset(0)]
        public byte u8;
        [FieldOffset(0)]
        public sbyte i8;
        [FieldOffset(0)]
        public UInt16 u16;
        [FieldOffset(0)]
        public Int16 i16;
        [FieldOffset(0)]
        public UInt32 u32;
        [FieldOffset(0)]
        public Int32 i32;
        [FieldOffset(0)]
        public float f32;
        [FieldOffset(0)]
        public UInt64 u64;
        [FieldOffset(0)]
        public Int64 i64;
        [FieldOffset(0)]
        public double f64;
    }
}
