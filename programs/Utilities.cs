using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using System.IO;
using GenericUtilitiesLib;
using System.Security.Cryptography;

namespace EmbedEmul
{
    public enum ByteOrder : byte
    {
        Invalid = 0,
        LittleEndian = 1,
        BigEndian = 2,
        Native = 3
    }

    public static class Utilities
    {
        ///// <summary>
        ///// Defines data types expected to be seen from a snapshot parameter.
        ///// </summary>
        //public enum DataType
        //{
        //    uint8, uint16, uint32, int16, int32, @float,
        //    B0, B1, B2, B3, B4, B5, B6, B7, B8, B9, B10, B11, B12, B13, B14, B15, B16,
        //    EB0, EB1, EB2, EB3, EB4, EB5, EB6, EB7, EB8, EB9, EB10, EB11, EB12, EB13, EB14, EB15, EB16,
        //    EB17, EB18, EB19, EB20, EB21, EB22, EB23, EB24, EB25, EB26, EB27, EB28, EB29, EB30, EB31, EB32
        //};

        ///// <summary>
        ///// Lookup table returning the Max Scaling per parameter type.
        ///// </summary>
        //public static Dictionary<DataType, int> Scaling = new Dictionary<DataType, int>()
        //{
        //    {DataType.uint8,8}, {DataType.uint16,16}, {DataType.uint32,32}, {DataType.int16,16}, {DataType.int32,31},
        //    {DataType.B0,15},   {DataType.B1,15},     {DataType.B2,15},     {DataType.B3,15},    {DataType.B4,15},
        //    {DataType.B5,15},   {DataType.B6,15},     {DataType.B7,15},     {DataType.B8,15},    {DataType.B9,15},
        //    {DataType.B10,15},  {DataType.B11,15},    {DataType.B12,15},    {DataType.B13,15},   {DataType.B14,15},
        //    {DataType.B15,15},  {DataType.B16,15},    {DataType.EB0,31},    {DataType.EB1,31},   {DataType.EB2,31},
        //    {DataType.EB3,31},  {DataType.EB4,31},    {DataType.EB5,31},    {DataType.EB6,31},   {DataType.EB7,31},
        //    {DataType.EB8,31},  {DataType.EB9,31},    {DataType.EB10,31},   {DataType.EB11,31},  {DataType.EB12,31},
        //    {DataType.EB13,31}, {DataType.EB14,31},   {DataType.EB15,31},   {DataType.EB16,31},  {DataType.EB17,31},
        //    {DataType.EB18,31}, {DataType.EB19,31},   {DataType.EB20,31},   {DataType.EB21,31},  {DataType.EB22,31},
        //    {DataType.EB23,31}, {DataType.EB24,31},   {DataType.EB25,31},   {DataType.EB26,31},  {DataType.EB27,31},
        //    {DataType.EB28,31}, {DataType.EB29,31},   {DataType.EB30,31},   {DataType.EB31,31},  {DataType.EB32,31},
        //    {DataType.@float,1}
        //};
        public static Regex regexWhiteSpaceSplit = new Regex("(?<=\\s*)[a-zA-Z0-9_\\.]+(?=\\s*)", RegexOptions.Compiled);

        public static string GetUNCPath(string path)
        {
            if (path.StartsWith(@"\\")) return path;
            else if (string.IsNullOrWhiteSpace(path)) return path;

            return path;

            /*
            DriveInfo driveInfo = new DriveInfo(path);

            if (driveInfo.DriveType == System.IO.DriveType.Network)
            {

                DirectoryInfo root = driveInfo.RootDirectory;
                string rootPath = root.FullName.Substring(0, 2);
                ManagementObject mo = new ManagementObject();
                mo.Path = new ManagementPath(string.Format("Win32_LogicalDisk='{0}'", rootPath));

                //DriveType 4 = Network Drive
                if (Convert.ToUInt32(mo["DriveType"]) == 4)
                {
                    return path.Replace(rootPath, Convert.ToString(mo["ProviderName"]));
                }
                else return path;
            }
            else
                return path;
             */
        }

        public static string GetTempPath()
        {
            string path = string.Format(@"{0}\EmbedEmul", Path.GetTempPath());

            if(!Directory.Exists(path)) Directory.CreateDirectory(path);
            return path;
        }

        public static string GetTempFileName(string origFilePath = null)
        {
            if(origFilePath == null)
                return string.Format(@"{0}\{1}.tmp", GetTempPath(), Guid.NewGuid());
            else
                return string.Format(@"{0}\{1}_{2}.tmp{3}", GetTempPath(), Path.GetFileNameWithoutExtension(origFilePath), Guid.NewGuid(), Path.GetExtension(origFilePath));
        }

        public static string GCStats()
        {
            StringBuilder build = new StringBuilder();
            build.Append(GC.CollectionCount(0));
            build.Append('|');
            build.Append(GC.CollectionCount(1));
            build.Append('|');
            build.Append(GC.CollectionCount(2));
            return build.ToString();
        }

        public static ByteOrder ResolveEndiananess(ByteOrder order)
        {
            return order == ByteOrder.Native ? (BitConverter.IsLittleEndian ? ByteOrder.LittleEndian : ByteOrder.BigEndian) : order;
        }

        public static bool TryParseEnum<TEnum>(string value, out TEnum returnVal)
            where TEnum : struct, IConvertible
        {
            if (!(typeof(TEnum).IsEnum))
                throw new NotSupportedException("T needs to be an enumerated type.");

            return Enum.TryParse<TEnum>(value, true, out returnVal);
        }

        #region Endian Conversion Utilities
        //public static long BytesToString(out string result, byte[] bytes, long startidx, long byteCount = -1)
        //{
        //    if(byteCount == -1) //take bytes until null terminator is found
        //    {
        //        StringBuilder builder = new StringBuilder();
        //        long ii = startidx;
        //        while(bytes[ii] != 0)
        //        {
        //            builder.Append((char)bytes[ii]);
        //            ii++;
        //        }
        //        ii++; //include null in count
        //        result = builder.ToString();
        //        byteCount = ii - startidx;
        //    }
        //    else //take number of bytes specified
        //    {
        //        char[] buffer = new char[byteCount];
        //        long end = startidx + byteCount;
        //        for(long ii = startidx, curr = 0; ii<end; ii++, curr++)
        //            buffer[curr] = (char)bytes[ii];
        //        result = new string(buffer);
        //    }
        //    return byteCount;
        //}
        /// <summary>
        /// Packs bytes using native byte order.
        /// </summary>
        /// <param name="bytes">Bytes to consume ordered natively</param>
        /// <param name="startindex">Start index</param>
        /// <param name="byteCount">Number of bytes to take, max 8</param>
        /// <returns></returns>
        //public static UInt64 PackBytes(byte[] bytes, int startidx, int byteCount, ByteOrder order = ByteOrder.Native)
        //{
        //    if (byteCount == 2) return BytesToUInt16(bytes, startidx, order);
        //    else if (byteCount == 4) return BytesToUInt32(bytes, startidx, order);
        //    else if (byteCount == 8) return BytesToUInt64(bytes, startidx, order);
        //    else if (byteCount == 1) return bytes[startidx];
        //    else throw new NotSupportedException("Unsupported byte count.");
        //}

        //public static Int16 BytesToInt16(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        //{
        //    return (Int16)BytesToUInt16(bytes, startidx, order);
        //}
        public static UInt16 BytesToUInt16(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        {
            if (order == ByteOrder.LittleEndian || (order == ByteOrder.Native && BitConverter.IsLittleEndian)) //array is in little endian order
            {
                return (UInt16)((UInt32)bytes[startidx] |
                    ((UInt32)bytes[startidx + 1] << 0x08));
            }
            else //array is in big endian order
            {
                return (UInt16)(((UInt32)bytes[startidx] << 0x08) |
                    (UInt32)bytes[startidx + 1]);
            }
        }

        public static float BytesToSingle(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        {
            UInt32FloatUnion union = new UInt32FloatUnion(BytesToUInt32(bytes, startidx, order));
            return union.f;
        }
        //public static Int32 BytesToInt32(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        //{
        //    return (Int32)BytesToUInt32(bytes, startidx, order);
        //}
        public static UInt32 BytesToUInt32(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        {
            if (order == ByteOrder.LittleEndian || (order == ByteOrder.Native && BitConverter.IsLittleEndian)) //array is in little endian order
            {
                return ((UInt32)bytes[startidx] |
                    ((UInt32)bytes[startidx + 1] << 0x08) |
                    ((UInt32)bytes[startidx + 2] << 0x10) |
                    ((UInt32)bytes[startidx + 3] << 0x18));
            }
            else //array is in big endian order
            {
                return (((UInt32)bytes[startidx] << 0x18) |
                    ((UInt32)bytes[startidx + 1] << 0x10) |
                    ((UInt32)bytes[startidx + 2] << 0x08) |
                    (UInt32)bytes[startidx + 3]);
            }
        }

        public static double BytesToDouble(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        {
            UInt64DoubleUnion union = new UInt64DoubleUnion(BytesToUInt64(bytes, startidx, order));
            return union.d;
        }
        //public static Int64 BytesToInt64(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        //{
        //    return (Int64)BytesToUInt64(bytes, startidx, order);
        //}
        public static UInt64 BytesToUInt64(byte[] bytes, long startidx, ByteOrder order = ByteOrder.Native)
        {
            if (order == ByteOrder.LittleEndian || (order == ByteOrder.Native && BitConverter.IsLittleEndian)) //array is in little endian order
            {
                return ((UInt64)bytes[startidx] |
                    ((UInt64)bytes[startidx + 1] << 0x08) |
                    ((UInt64)bytes[startidx + 2] << 0x10) |
                    ((UInt64)bytes[startidx + 3] << 0x18) |
                    ((UInt64)bytes[startidx + 4] << 0x20) |
                    ((UInt64)bytes[startidx + 5] << 0x28) |
                    ((UInt64)bytes[startidx + 6] << 0x30) |
                    ((UInt64)bytes[startidx + 7] << 0x38));
            }
            else //array is in big endian order
            {
                return (((UInt64)bytes[startidx] << 0x38) |
                    ((UInt64)bytes[startidx + 1] << 0x30) |
                    ((UInt64)bytes[startidx + 2] << 0x28) |
                    ((UInt64)bytes[startidx + 3] << 0x20) |
                    ((UInt64)bytes[startidx + 4] << 0x18) |
                    ((UInt64)bytes[startidx + 5] << 0x10) |
                    ((UInt64)bytes[startidx + 6] << 0x08) |
                    (UInt64)bytes[startidx + 7]);
            }
        }

        //public static UInt32 BytesToULEB128(byte[] bytes, long startidx, out UInt64 value)
        //{
        //    UInt32 pos = 0;
        //    byte val;
        //    value = 0;
        //    do
        //    {
        //        val = bytes[startidx + pos];
        //        value |= (UInt64)((UInt64)(val & 0x7f) << (int)(7 * pos));
        //        pos++;

        //    } while ((val & 0x80) == 0x80);

        //    return pos;
        //}

        //public static UInt32 BytesToSLEB128(byte[] bytes, long startidx, out Int64 value)
        //{
        //    UInt32 pos = 0;
        //    byte val;
        //    int shift = 0;
        //    value = 0;

        //    do
        //    {
        //        val = bytes[startidx + pos];
        //        value |= (val & 0x7f) << shift;
        //        shift += 7;
        //        pos++;
        //    } while ((val & 0x80) == 0x80);

        //    if (shift < 64 && (val & 0x40) == 0x40)
        //        value |= -(1 << shift);

        //    return pos;
        //}

        //public static void TakeBytes(byte[] bytes, int startindex, ref byte[] store, int storeidx, int count, ByteOrder asorder)
        //{
        //    int end = startindex + count;
        //    bool nativeOrder = Utilities.IsNativeByteOrder(asorder);
        //    int dir = nativeOrder ? 1 : -1;
        //    storeidx = nativeOrder ? storeidx : storeidx + count - 1;

        //    for (; startindex < end; startindex++)
        //    {
        //        store[storeidx] = bytes[startindex];
        //        storeidx += dir;
        //    }
        //}

        //public static void GetFloatingBytes(string value, long byteCount, ref byte[] buffer, long startidx, ByteOrder order)
        //{
        //    if(byteCount == 4)
        //        GetBytes(float.Parse(value),buffer,startidx,order);
        //    else if (byteCount == 8)
        //        GetBytes(double.Parse(value),buffer,startidx,order);
        //    else throw new NotSupportedException(string.Format("Utilities.GetFloatingBytes does not support {0} bytes.",byteCount));
        //}


        //public static void GetIntegerBytes(string value, long byteCount, bool signed, ref byte[] buffer, long startidx, ByteOrder order)
        //{
        //    if (byteCount == 1)
        //    {
        //        buffer[startidx] = signed ? (byte)sbyte.Parse(value) : byte.Parse(value);
        //        return;
        //    }
        //    else if (byteCount == 2)
        //        if (signed)
        //            GetBytes(Int16.Parse(value), buffer, startidx, order);
        //        else
        //            GetBytes(UInt16.Parse(value), buffer, startidx, order);
        //    else if (byteCount == 4)
        //        if (signed)
        //            GetBytes(Int32.Parse(value), buffer, startidx, order);
        //        else
        //            GetBytes(UInt32.Parse(value), buffer, startidx, order);
        //    else if (byteCount == 8)
        //        if (signed)
        //            GetBytes(Int64.Parse(value), buffer, startidx, order);
        //        else
        //            GetBytes(UInt64.Parse(value), buffer, startidx, order);
        //    else throw new NotSupportedException(string.Format("Utilites.GetIntegerBytes does not support {0} bytes.", byteCount));
        //}
        //public static void GetIntegerBytes(double value, long byteCount, bool signed, ref byte[] buffer, long startidx, ByteOrder order)
        //{
        //    if (byteCount == 1)
        //    {
        //        if (signed)
        //        {
        //            value = (value > sbyte.MaxValue) ? sbyte.MaxValue : ((value < sbyte.MinValue) ? sbyte.MinValue : value);
        //            buffer[startidx] = (byte)(sbyte)value;
        //            return;
        //        }
        //        else
        //        {
        //            value = (value > byte.MaxValue) ? byte.MaxValue : ((value < byte.MinValue) ? byte.MinValue : value);
        //            buffer[startidx] = (byte)value;
        //            return;
        //        }
        //    }
        //    else if (byteCount == 2)
        //        if (signed)
        //        {
        //            value = (value > Int16.MaxValue) ? Int16.MaxValue : ((value < Int16.MinValue) ? Int16.MinValue : value);
        //            GetBytes((Int16)value, buffer, startidx, order);
        //        }
        //        else
        //        {
        //            value = (value > UInt16.MaxValue) ? UInt16.MaxValue : ((value < UInt16.MinValue) ? UInt16.MinValue : value);
        //            GetBytes((UInt16)value, buffer, startidx, order);
        //        }
        //    else if (byteCount == 4)
        //        if (signed)
        //        {
        //            value = (value > Int32.MaxValue) ? Int32.MaxValue : ((value < Int32.MinValue) ? Int32.MinValue : value);
        //            GetBytes((Int32)value, buffer, startidx, order);
        //        }
        //        else
        //        {
        //            value = (value > UInt32.MaxValue) ? UInt32.MaxValue : ((value < UInt32.MinValue) ? UInt32.MinValue : value);
        //            GetBytes((UInt32)value, buffer, startidx, order);
        //        }
        //    else if (byteCount == 8)
        //        if (signed)
        //        {
        //            value = (value > Int64.MaxValue) ? Int64.MaxValue : ((value < Int64.MinValue) ? Int64.MinValue : value);
        //            GetBytes((Int64)value, buffer, startidx, order);
        //        }
        //        else
        //        {
        //            value = (value > UInt64.MaxValue) ? UInt64.MaxValue : ((value < UInt64.MinValue) ? UInt64.MinValue : value);
        //            GetBytes((UInt64)value, buffer, startidx, order);
        //        }
        //    else throw new NotSupportedException(string.Format("Utilites.GetIntegerBytes does not support {0} bytes.", byteCount));
        //}

        //public static void GetBytes(sbyte value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    buffer[startidx] = (byte)value;
        //}
        //public static void GetBytes(byte value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    buffer[startidx] = value;
        //}

        //public static void GetBytes(Int16 value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    GetBytes((UInt16)value, buffer, startidx, order);
        //}
        //public static void GetBytes(UInt16 value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    //Check if order reversal is needed
        //    if (order == ByteOrder.LittleEndian || (order == ByteOrder.Native && BitConverter.IsLittleEndian))
        //    {
        //        buffer[startidx] = (byte)value;
        //        buffer[startidx + 1] = (byte)(value >> 0x8);
        //    }
        //    else
        //    {
        //        buffer[startidx] = (byte)(value >> 0x8);
        //        buffer[startidx + 1] = (byte)value;
        //    }
        //}

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
        //public static void GetBytes(float value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    UInt32FloatUnion union = new UInt32FloatUnion(value);
        //    GetBytes(union.u, buffer, startidx, order);
        //}
        //public static void GetBytes(Int32 value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    GetBytes((UInt32)value, buffer, startidx, order);
        //}
        //public static void GetBytes(UInt32 value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    //Check if order reversal is needed
        //    if (order == ByteOrder.LittleEndian || (order == ByteOrder.Native && BitConverter.IsLittleEndian))
        //    {
        //        buffer[startidx] = (byte)value;
        //        buffer[startidx + 1] = (byte)(value >> 0x08);
        //        buffer[startidx + 2] = (byte)(value >> 0x10);
        //        buffer[startidx + 3] = (byte)(value >> 0x18);
        //    }
        //    else
        //    {
        //        buffer[startidx] = (byte)(value >> 0x18);
        //        buffer[startidx + 1] = (byte)(value >> 0x10);
        //        buffer[startidx + 2] = (byte)(value >> 0x08);
        //        buffer[startidx + 3] = (byte)value;
        //    }
        //}

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

        //public static void GetBytes(double value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    UInt64DoubleUnion union = new UInt64DoubleUnion(value);
        //    GetBytes(union.u, buffer, startidx, order);
        //}

        //public static void GetBytes(Int64 value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    GetBytes((UInt64)value, buffer, startidx, order);
        //}

        //public static void GetBytes(UInt64 value, byte[] buffer, long startidx, ByteOrder order)
        //{
        //    //Check if order reversal is needed
        //    if (order == ByteOrder.LittleEndian || (order == ByteOrder.Native && BitConverter.IsLittleEndian))
        //    {
        //        buffer[startidx] = (byte)value;
        //        buffer[startidx + 1] = (byte)(value >> 0x08);
        //        buffer[startidx + 2] = (byte)(value >> 0x10);
        //        buffer[startidx + 3] = (byte)(value >> 0x18);
        //        buffer[startidx + 4] = (byte)(value >> 0x20);
        //        buffer[startidx + 5] = (byte)(value >> 0x28);
        //        buffer[startidx + 6] = (byte)(value >> 0x30);
        //        buffer[startidx + 7] = (byte)(value >> 0x38);
        //    }
        //    else
        //    {
        //        buffer[startidx] = (byte)(value >> 0x38);
        //        buffer[startidx + 1] = (byte)(value >> 0x30);
        //        buffer[startidx + 2] = (byte)(value >> 0x28);
        //        buffer[startidx + 3] = (byte)(value >> 0x20);
        //        buffer[startidx + 4] = (byte)(value >> 0x18);
        //        buffer[startidx + 5] = (byte)(value >> 0x10);
        //        buffer[startidx + 6] = (byte)(value >> 0x08);
        //        buffer[startidx + 7] = (byte)value;
        //    }
        //}

        //public static long GetBytes(string value, byte[] buffer, long startidx, ByteOrder order, long maxLen = -1)
        //{
        //    long count = 0;
        //    if (maxLen == -1)
        //    {
        //        foreach (byte c in ASCIIEncoding.UTF8.GetBytes(value))
        //        {
        //            buffer[startidx + count] = c;
        //            count++;
        //        }
        //        buffer[startidx + count] = 0;
        //        count++;
        //    }
        //    else
        //    {
        //        byte[] bytes = ASCIIEncoding.UTF8.GetBytes(value);
        //        long end;
        //        if (bytes.LongLength > maxLen)
        //            end = maxLen;
        //        else
        //            end = bytes.LongLength;

        //        for(long ii = 0; ii < end; ii++)
        //        {
        //            buffer[startidx + ii] = bytes[ii];
        //        }

        //        //Fill in with 0's
        //        if(end < maxLen)
        //        {
        //            for(; end < maxLen; end++)
        //            {
        //                buffer[startidx + end] = 0;
        //            }
        //        }
        //        count = maxLen;
        //    }
        //    return count;
        //}

        public static void CheckArrayLength<T>(ref T[] array, long curridx, long countToAdd)
        {
            //Check array length and resize if needed;
            if (array.Length - curridx < countToAdd)
            {
                T[] buffer = new T[curridx + countToAdd];
                array.CopyTo(buffer, 0);
                array = buffer;
            }
        }

        //public static void CheckByteOrder(byte[] bytes, int startidx, int count, ByteOrder suppliedByteOrder)
        //{
        //    if (IsNativeByteOrder(suppliedByteOrder)) return;
        //    else ReverseByteOrder(bytes, startidx, count);
        //}

        public static bool IsNativeByteOrder(ByteOrder order)
        {
            if (order == ByteOrder.Native)
                return true;
            else
                return (BitConverter.IsLittleEndian && order == ByteOrder.LittleEndian) || //both little
                    (!BitConverter.IsLittleEndian && order == ByteOrder.BigEndian); //both big
        }

        //public static ulong Bytes2UInt(byte[] bytes, long startidx, int len, ByteOrder order = ByteOrder.Native)
        //{

        //    if (len == 2)
        //        return Utilities.BytesToUInt16(bytes, startidx, order);
        //    else if (len == 4)
        //        return Utilities.BytesToUInt32(bytes, startidx, order);
        //    else if (len == 8)
        //        return Utilities.BytesToUInt64(bytes, startidx, order);
        //    else if (len == 1)
        //        return (ulong)bytes[startidx];
        //    else throw new NotSupportedException(string.Format("{0} byte length unsupported.", len));
        //}
        //private static void ReverseByteOrder(byte[] bytes, int startidx, int count)
        //{
        //    int end = startidx + count;
        //    byte[] buffer = new byte[count];
        //    int idx = count - 1;

        //    for (int i = startidx; i < end; i++)
        //    {
        //        buffer[idx] = bytes[i];
        //        idx--;
        //    }

        //    buffer.CopyTo(bytes, startidx);
        //}
        #endregion

        #region Hex Utilities

        public static int Dec2Byte(string dec)
        {
            int val = int.Parse(dec);
            return val < 0 ? val + 256 : val;
        }

        public static long Hex2Dec(string hex)
        {
            return Convert.ToInt64(hex, 16);
        }

        public static string Byte2Hex(byte val)
        {
            return Byte2HexTable[val];
        }

        public static float Hex2Float(string hex)
        {
            //convert string representation to a uint value
            uint hexnum = uint.Parse(hex, System.Globalization.NumberStyles.AllowHexSpecifier);
            //get representative bytes
            byte[] hexbytes = BitConverter.GetBytes(hexnum);
            //convert to float and return
            return BitConverter.ToSingle(hexbytes, 0);
        }

        public static string DisplayBytes(long value, int numOfBytes, string delimiter = ".", string format = "0")
        {
            Debug.Assert(numOfBytes<8,"Number of bytes should be no more than 8.");

            long currentbyte = 0;
            int shift = numOfBytes - 1;
            long mask = 255<<8*(numOfBytes-1);
            StringBuilder result = new StringBuilder();
            for(int i = 0; i< numOfBytes; i++)
            {
                currentbyte = (value & mask) >> (int)(8*(shift-i));
                result.Append(((byte)currentbyte).ToString(format));
                if(i<numOfBytes-1) result.Append(delimiter);
                mask >>= 8;
            }
            return result.ToString();
        }

        public static UInt64 Clamp(UInt64 value, long byteSize, long bitSize = 0)
        {
            if (byteSize > 8)
                throw new NotSupportedException();

            if (bitSize != 0)
                throw new NotImplementedException();

            if (value < UnsignedMinMax[byteSize, 0])
                value = UnsignedMinMax[byteSize, 0];
            else if (value > UnsignedMinMax[byteSize, 1])
                value = UnsignedMinMax[byteSize, 1];

            return (UInt64)value;

        }

        public static Int64 Clamp(Int64 value, long byteSize, long bitSize = 0)
        {
            if (byteSize > 8)
                throw new NotSupportedException();

            if (bitSize != 0)
                throw new NotImplementedException();

            if (value < SignedMinMax[byteSize, 0])
                value = SignedMinMax[byteSize, 0];
            else if (value > SignedMinMax[byteSize, 1])
                value = SignedMinMax[byteSize, 1];

            return (Int64)value;
        }

        public static string[] Byte2HexTable =
        {
            "00","01","02","03","04","05","06","07","08","09",  //0-9
            "0A","0B","0C","0D","0E","0F","10","11","12","13",  //10-19
            "14","15","16","17","18","19","1A","1B","1C","1D",  //20-29
            "1E","1F","20","21","22","23","24","25","26","27",  //30-39
            "28","29","2A","2B","2C","2D","2E","2F","30","31",  //40-49
            "32","33","34","35","36","37","38","39","3A","3B",  //50-59
            "3C","3D","3E","3F","40","41","42","43","44","45",  //60-69
            "46","47","48","49","4A","4B","4C","4D","4E","4F",  //70-79
            "50","51","52","53","54","55","56","57","58","59",  //80-89
            "5A","5B","5C","5D","5E","5F","60","61","62","63",  //90-99
            "64","65","66","67","68","69","6A","6B","6C","6D",  //100-109
            "6E","6F","70","71","72","73","74","75","76","77",  //110-119
            "78","79","7A","7B","7C","7D","7E","7F","80","81",  //120-129
            "82","83","84","85","86","87","88","89","8A","8B",  //130-139
            "8C","8D","8E","8F","90","91","92","93","94","95",  //140-149
            "96","97","98","99","9A","9B","9C","9D","9E","9F",  //150-159
            "A0","A1","A2","A3","A4","A5","A6","A7","A8","A9",  //160-169
            "AA","AB","AC","AD","AE","AF","B0","B1","B2","B3",  //170-179
            "B4","B5","B6","B7","B8","B9","BA","BB","BC","BD",  //180-189
            "BE","BF","C0","C1","C2","C3","C4","C5","C6","C7",  //190-199
            "C8","C9","CA","CB","CC","CD","CE","CF","D0","D1",  //200-209
            "D2","D3","D4","D5","D6","D7","D8","D9","DA","DB",  //210-219
            "DC","DD","DE","DF","E0","E1","E2","E3","E4","E5",  //220-229
            "E6","E7","E8","E9","EA","EB","EC","ED","EE","EF",  //230-239
            "F0","F1","F2","F3","F4","F5","F6","F7","F8","F9",  //240-249
            "FA","FB","FC","FD","FE","FF",                      //250-256
        };

        public static char[] Byte2AsciiTable =
        {
            '.','.','.','.','.','.','.','.','.','.',  //0-9
            '.','.','.','.','.','.','.','.','.','.',  //10-19
            '.','.','.','.','.','.','.','.','.','.',  //20-29
            '.','.','.','!','"','#','$','%','&','\'',  //30-39
            '(',')','*','+',',','-','.','/','0','1',  //40-49
            '2','3','4','5','6','7','8','9',':',';',  //50-59
            '<','=','>','?','@','A','B','C','D','E',  //60-69
            'F','G','H','I','J','K','L','M','N','O',  //70-79
            'P','Q','R','S','T','U','V','W','X','Y',  //80-89
            'Z','[','\\',']','^','_','`','a','b','c',  //90-99
            'd','e','f','g','h','i','j','k','l','m',  //100-109
            'n','o','p','q','r','s','t','u','v','w',  //110-119
            'x','y','z','{','|','}','~','.','Ç','ü',  //120-129
            'é','â','ä','à','å','ç','ê','ë','è','ï',  //130-139
            'î','ì','Ä','Å','É','æ','Æ','ô','ö','ò',  //140-149
            'û','ù','ÿ','Ö','Ü','ø','£','Ø','×','ƒ',  //150-159
            'á','í','ó','ú','ñ','Ñ','ª','º','¿','®',  //160-169
            '¬','½','¼','¡','«','»','░','▒','▓','│',  //170-179
            '┤','Á','Â','À','©','╣','║','╗','╝','¢',  //180-189
            '¥','┐','└','┴','┬','├','─','┼','ã','Ã',  //190-199
            '╚','╔','╩','╦','╠','═','╬','¤','ð','Ð',  //200-209
            'Ê','Ë','È','ı','Í','Î','Ï','┘','┌','█',  //210-219
            '▄','¦','Ì','▀','Ó','ß','Ô','Ò','õ','Õ',  //220-229
            'µ','þ','Þ','Ú','Û','Ù','ý','Ý','¯','´',  //230-239
            '≡','±','‗','¾','¶','§','÷','¸','°','¨',  //240-249
            '·','¹','³','²','■','.',                      //250-256
        };

        public static string[] Byte2DecTable =
        {
            "0","1","2","3","4","5","6","7","8","9", //0-9
            "10","11","12","13","14","15","16","17","18","19", //10-19
            "20","21","22","23","24","25","26","27","28","29", //20-29
            "30","31","32","33","34","35","36","37","38","39", //30-39
            "40","41","42","43","44","45","46","47","48","49", //40-49
            "50","51","52","53","54","55","56","57","58","59", //50-59
            "60","61","62","63","64","65","66","67","68","69", //60-69
            "70","71","72","73","74","75","76","77","78","79", //70-79
            "80","81","82","83","84","85","86","87","88","89", //80-89
            "90","91","92","93","94","95","96","97","98","99", //90-99
            "100","101","102","103","104","105","106","107","108","109", //100-109
            "110","111","112","113","114","115","116","117","118","119", //110-119
            "120","121","122","123","124","125","126","127","128","129", //120-129
            "130","131","132","133","134","135","136","137","138","139", //130-139
            "140","141","142","143","144","145","146","147","148","149", //140-149
            "150","151","152","153","154","155","156","157","158","159", //150-159
            "160","161","162","163","164","165","166","167","168","169", //160-169
            "170","171","172","173","174","175","176","177","178","179", //170-179
            "180","181","182","183","184","185","186","187","188","189", //180-189
            "190","191","192","193","194","195","196","197","198","199", //190-199
            "200","201","202","203","204","205","206","207","208","209", //200-209
            "210","211","212","213","214","215","216","217","218","219", //210-219
            "220","221","222","223","224","225","226","227","228","229", //220-229
            "230","231","232","233","234","235","236","237","238","239", //230-239
            "240","241","242","243","244","245","246","247","248","249", //240-249
            "250","251","252","253","254","255",
        };

        public static char[] Nibble2HexTable =
        {
            '0','1','2','3','4','5','6','7','8','9',
            'A','B','C','D','E','F'
        };

        public static byte[] NibbleHex2ValueTable =
        {
            0,0,0,0,0,0,0,0,0,0, //0-9
            0,0,0,0,0,0,0,0,0,0, //10-19
            0,0,0,0,0,0,0,0,0,0, //20-29
            0,0,0,0,0,0,0,0,0,0, //30-39
            0,0,0,0,0,0,0,0,0,1, //40-49
            2,3,4,5,6,7,8,9,0,0, //50-59
            0,0,0,0,0,10,11,12,13,14, //60-69
            15,0,0,0,0,0,0,0,0,0, //70-79
            0,0,0,0,0,0,0,0,0,0, //80-89
            0,0,0,0,0,0,0,10,11,12, //90-99
            13,14,15,0,0,0,0,0,0,0, //100-109
            0,0,0,0,0,0,0,0,0,0, //110-119
            0,0,0,0,0,0,0,0 //120-127 ... 128 elements
        };

        public static UInt64[] pow10 =
        {
            1, //0
            10, //1
            100, //2
            1000, //3
            10000, //4
            100000, //5
            1000000, //6
            10000000, //7
            100000000, //8
            1000000000, //9
            10000000000, //10
            100000000000, //11
            1000000000000, //12
            10000000000000, //13
            100000000000000, //14
            1000000000000000, //15
            10000000000000000, //16
            100000000000000000, //17
            1000000000000000000, //18
            10000000000000000000 //19
        };

        internal static UInt64[,] UnsignedMinMax =
        {
            {0,0 },
            {byte.MinValue, byte.MaxValue },
            {UInt16.MinValue, UInt16.MaxValue },
            {UInt32.MinValue, UInt32.MaxValue & 0x00FFFFFF},
            {UInt32.MinValue, UInt32.MaxValue },
            {UInt64.MinValue, UInt64.MaxValue & 0x000000FFFFFFFFFF},
            {UInt64.MinValue, UInt64.MaxValue & 0x0000FFFFFFFFFFFF},
            {UInt64.MinValue, UInt64.MaxValue & 0x00FFFFFFFFFFFFFF},
            {UInt64.MinValue, UInt64.MaxValue }
        };

        internal static Int64[,] SignedMinMax =
        {
            {0,0 },
            {sbyte.MinValue, sbyte.MaxValue },
            {Int16.MinValue, Int16.MaxValue },
            {Int32.MinValue & 0x10FFFFFF, Int32.MaxValue & 0x00EFFFFF},
            {Int32.MinValue, Int32.MaxValue },
            {Int64.MinValue & 0x100000FFFFFFFFFF, Int64.MaxValue & 0x000000EFFFFFFFFF},
            {Int64.MinValue & 0x1000FFFFFFFFFFFF, Int64.MaxValue & 0x0000EFFFFFFFFFFF},
            {Int64.MinValue & 0x10FFFFFFFFFFFFFF, Int64.MaxValue & 0x00EFFFFFFFFFFFFF},
            {Int64.MinValue, Int64.MaxValue }
        };
        #endregion
        #region CRC and Checksum Utilities

        public static byte[] ParityTable = new byte[256]
        {
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1,
           0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0
        };

        public static ushort[] FcsTable = new ushort[256]
        {
            0x0000, 0x1189, 0x2312, 0x329b, 0x4624, 0x57ad, 0x6536, 0x74bf,
            0x8c48, 0x9dc1, 0xaf5a, 0xbed3, 0xca6c, 0xdbe5, 0xe97e, 0xf8f7,
            0x1081, 0x0108, 0x3393, 0x221a, 0x56a5, 0x472c, 0x75b7, 0x643e,
            0x9cc9, 0x8d40, 0xbfdb, 0xae52, 0xdaed, 0xcb64, 0xf9ff, 0xe876,
            0x2102, 0x308b, 0x0210, 0x1399, 0x6726, 0x76af, 0x4434, 0x55bd,
            0xad4a, 0xbcc3, 0x8e58, 0x9fd1, 0xeb6e, 0xfae7, 0xc87c, 0xd9f5,
            0x3183, 0x200a, 0x1291, 0x0318, 0x77a7, 0x662e, 0x54b5, 0x453c,
            0xbdcb, 0xac42, 0x9ed9, 0x8f50, 0xfbef, 0xea66, 0xd8fd, 0xc974,
            0x4204, 0x538d, 0x6116, 0x709f, 0x0420, 0x15a9, 0x2732, 0x36bb,
            0xce4c, 0xdfc5, 0xed5e, 0xfcd7, 0x8868, 0x99e1, 0xab7a, 0xbaf3,
            0x5285, 0x430c, 0x7197, 0x601e, 0x14a1, 0x0528, 0x37b3, 0x263a,
            0xdecd, 0xcf44, 0xfddf, 0xec56, 0x98e9, 0x8960, 0xbbfb, 0xaa72,
            0x6306, 0x728f, 0x4014, 0x519d, 0x2522, 0x34ab, 0x0630, 0x17b9,
            0xef4e, 0xfec7, 0xcc5c, 0xddd5, 0xa96a, 0xb8e3, 0x8a78, 0x9bf1,
            0x7387, 0x620e, 0x5095, 0x411c, 0x35a3, 0x242a, 0x16b1, 0x0738,
            0xffcf, 0xee46, 0xdcdd, 0xcd54, 0xb9eb, 0xa862, 0x9af9, 0x8b70,
            0x8408, 0x9581, 0xa71a, 0xb693, 0xc22c, 0xd3a5, 0xe13e, 0xf0b7,
            0x0840, 0x19c9, 0x2b52, 0x3adb, 0x4e64, 0x5fed, 0x6d76, 0x7cff,
            0x9489, 0x8500, 0xb79b, 0xa612, 0xd2ad, 0xc324, 0xf1bf, 0xe036,
            0x18c1, 0x0948, 0x3bd3, 0x2a5a, 0x5ee5, 0x4f6c, 0x7df7, 0x6c7e,
            0xa50a, 0xb483, 0x8618, 0x9791, 0xe32e, 0xf2a7, 0xc03c, 0xd1b5,
            0x2942, 0x38cb, 0x0a50, 0x1bd9, 0x6f66, 0x7eef, 0x4c74, 0x5dfd,
            0xb58b, 0xa402, 0x9699, 0x8710, 0xf3af, 0xe226, 0xd0bd, 0xc134,
            0x39c3, 0x284a, 0x1ad1, 0x0b58, 0x7fe7, 0x6e6e, 0x5cf5, 0x4d7c,
            0xc60c, 0xd785, 0xe51e, 0xf497, 0x8028, 0x91a1, 0xa33a, 0xb2b3,
            0x4a44, 0x5bcd, 0x6956, 0x78df, 0x0c60, 0x1de9, 0x2f72, 0x3efb,
            0xd68d, 0xc704, 0xf59f, 0xe416, 0x90a9, 0x8120, 0xb3bb, 0xa232,
            0x5ac5, 0x4b4c, 0x79d7, 0x685e, 0x1ce1, 0x0d68, 0x3ff3, 0x2e7a,
            0xe70e, 0xf687, 0xc41c, 0xd595, 0xa12a, 0xb0a3, 0x8238, 0x93b1,
            0x6b46, 0x7acf, 0x4854, 0x59dd, 0x2d62, 0x3ceb, 0x0e70, 0x1ff9,
            0xf78f, 0xe606, 0xd49d, 0xc514, 0xb1ab, 0xa022, 0x92b9, 0x8330,
            0x7bc7, 0x6a4e, 0x58d5, 0x495c, 0x3de3, 0x2c6a, 0x1ef1, 0x0f78
        };

        //Use in conjunction with the following table if wanting table lookup calculation for
        //GTIS documents... otherwise just use CRC_GTIS_DOC
        //public static ushort CRC_GTIS_DOC_TABLE(byte[] bytes, ushort crc = 0x0000)
        //{
        //    foreach (byte value in bytes)
        //        if (value >= 0x20)
        //            crc = (ushort)(((crc >> 7) & 0x1fe) ^ XcalFCSTable[(crc ^ value) & 0xFF]);
        //    return crc;
        //}
        //static ushort[] XcalFCSTable = new ushort[256]
        //{
        //    0x0000, 0x80C2, 0x8182, 0x0140, 0x8302, 0x03C0, 0x0280, 0x8242,
        //    0x8602, 0x06C0, 0x0780, 0x8742, 0x0500, 0x85C2, 0x8482, 0x0440,
        //    0x8C02, 0x0CC0, 0x0D80, 0x8D42, 0x0F00, 0x8FC2, 0x8E82, 0x0E40,
        //    0x0A00, 0x8AC2, 0x8B82, 0x0B40, 0x8902, 0x09C0, 0x0880, 0x8842,
        //    0x9802, 0x18C0, 0x1980, 0x9942, 0x1B00, 0x9BC2, 0x9A82, 0x1A40,
        //    0x1E00, 0x9EC2, 0x9F82, 0x1F40, 0x9D02, 0x1DC0, 0x1C80, 0x9C42,
        //    0x1400, 0x94C2, 0x9582, 0x1540, 0x9702, 0x17C0, 0x1680, 0x9642,
        //    0x9202, 0x12C0, 0x1380, 0x9342, 0x1100, 0x91C2, 0x9082, 0x1040,
        //    0xB002, 0x30C0, 0x3180, 0xB142, 0x3300, 0xB3C2, 0xB282, 0x3240,
        //    0x3600, 0xB6C2, 0xB782, 0x3740, 0xB502, 0x35C0, 0x3480, 0xB442,
        //    0x3C00, 0xBCC2, 0xBD82, 0x3D40, 0xBF02, 0x3FC0, 0x3E80, 0xBE42,
        //    0xBA02, 0x3AC0, 0x3B80, 0xBB42, 0x3900, 0xB9C2, 0xB882, 0x3840,
        //    0x2800, 0xA8C2, 0xA982, 0x2940, 0xAB02, 0x2BC0, 0x2A80, 0xAA42,
        //    0xAE02, 0x2EC0, 0x2F80, 0xAF42, 0x2D00, 0xADC2, 0xAC82, 0x2C40,
        //    0xA402, 0x24C0, 0x2580, 0xA542, 0x2700, 0xA7C2, 0xA682, 0x2640,
        //    0x2200, 0xA2C2, 0xA382, 0x2340, 0xA102, 0x21C0, 0x2080, 0xA042,
        //    0xE002, 0x60C0, 0x6180, 0xE142, 0x6300, 0xE3C2, 0xE282, 0x6240,
        //    0x6600, 0xE6C2, 0xE782, 0x6740, 0xE502, 0x65C0, 0x6480, 0xE442,
        //    0x6C00, 0xECC2, 0xED82, 0x6D40, 0xEF02, 0x6FC0, 0x6E80, 0xEE42,
        //    0xEA02, 0x6AC0, 0x6B80, 0xEB42, 0x6900, 0xE9C2, 0xE882, 0x6840,
        //    0x7800, 0xF8C2, 0xF982, 0x7940, 0xFB02, 0x7BC0, 0x7A80, 0xFA42,
        //    0xFE02, 0x7EC0, 0x7F80, 0xFF42, 0x7D00, 0xFDC2, 0xFC82, 0x7C40,
        //    0xF402, 0x74C0, 0x7580, 0xF542, 0x7700, 0xF7C2, 0xF682, 0x7640,
        //    0x7200, 0xF2C2, 0xF382, 0x7340, 0xF102, 0x71C0, 0x7080, 0xF042,
        //    0x5000, 0xD0C2, 0xD182, 0x5140, 0xD302, 0x53C0, 0x5280, 0xD242,
        //    0xD602, 0x56C0, 0x5780, 0xD742, 0x5500, 0xD5C2, 0xD482, 0x5440,
        //    0xDC02, 0x5CC0, 0x5D80, 0xDD42, 0x5F00, 0xDFC2, 0xDE82, 0x5E40,
        //    0x5A00, 0xDAC2, 0xDB82, 0x5B40, 0xD902, 0x59C0, 0x5880, 0xD842,
        //    0xC802, 0x48C0, 0x4980, 0xC942, 0x4B00, 0xCBC2, 0xCA82, 0x4A40,
        //    0x4E00, 0xCEC2, 0xCF82, 0x4F40, 0xCD02, 0x4DC0, 0x4C80, 0xCC42,
        //    0x4400, 0xC4C2, 0xC582, 0x4540, 0xC702, 0x47C0, 0x4680, 0xC642,
        //    0xC202, 0x42C0, 0x4380, 0xC342, 0x4100, 0xC1C2, 0xC082, 0x4040
        //};

        /// <summary>
        /// One flavor of CRC
        /// </summary>
        /// <param name="bytes"></param>
        /// <returns></returns>
        public static ushort CRC_Parity(byte[] bytes, ushort crc = 0x0000, long startidx = 0, long count = -1) //0xc001
        {
            int c;

            if (count == -1) count = bytes.LongLength + startidx;
            else count = count + startidx;

            for(long ii = startidx; ii < count; ii++)
                if (bytes[ii] >= 0x20)
                {
                    c = (crc ^ bytes[ii]) & 0x00FF;
                    crc = (ushort)(crc >> 8);
                    if (ParityTable[(byte)c] > 0)
                        crc = (ushort)(crc ^ 0xc001); //0x8001
                    c = c << 6;
                    crc = (ushort)(crc ^ c);
                    crc = (ushort)(crc << 1);
                    crc = (ushort)(crc ^ c);
                }

            return crc;
        }

        /// <summary>
        /// Generic 16-bit CRC for calculating code block crcs CCITT?.
        /// </summary>
        /// <param name="bytes"></param>
        /// <param name="crc"></param>
        /// <returns></returns>
        public static ushort CRC16(IEnumerable<byte> bytes, ushort crc = 0x0000)
        {
            foreach (byte value in bytes)
                crc = (ushort)((crc >> 8) ^ FcsTable[(crc ^ value) & 0xFF]);

            return crc;

        }


        /// <summary>
        /// Calcualtes a variable length checksum (given by numOfBytes) on a given byte stream.
        /// </summary>
        /// <param name="bytes"></param>
        /// <param name="checksum"></param>
        /// <param name="numOfBytes"></param>
        /// <returns></returns>
        public static ulong Checksum(IEnumerable<byte> bytes, ulong checksum = 0, int numOfBytes = 2, ByteOrder order = ByteOrder.BigEndian)
        {
            if (numOfBytes > 8) throw new NotSupportedException("Checksum result greater than 8 bytes not supported.");
            int numOfBits = numOfBytes << 3;
            ulong mask = numOfBytes == 8 ? ulong.MaxValue : (1UL << numOfBits) - 1;
            ulong fullval = 0;
            numOfBits = numOfBits - 1;
            int shift;
            if (order == ByteOrder.LittleEndian)
            {
                shift = 0;
                foreach (byte value in bytes)
                {
                    fullval = fullval + (((UInt64)value) << shift);
                    shift = (shift + 8) & numOfBits;
                    if (shift == 0)
                    {
                        checksum = (checksum + fullval) & mask;
                        fullval = 0;
                    }
                }
            }
            else if (order == ByteOrder.BigEndian)
            {
                shift = numOfBits - 7;
                foreach (byte value in bytes)
                {
                    fullval = fullval + (((UInt64)value) << shift);
                    shift = (shift - 8) & numOfBits;
                    if (shift == numOfBits - 7)
                    {
                        checksum = (checksum + fullval) & mask;
                        fullval = 0;
                    }
                }

            }

            return (checksum + fullval) & mask;
        }
        #endregion

        public static T CreateFile<T>(params object[] args)
        {
            //Create an array of the provided types of objects
            Type[] constructorDefinition = new Type[args.Length];
            for (int ii = 0; ii < args.Length; ii++) constructorDefinition[ii] = args[ii].GetType();
            //Get the relevant constructor from the desired type
            System.Reflection.ConstructorInfo constructor = typeof(T).GetConstructor(constructorDefinition);
            //Sanity check
            if (constructor == null) throw new NotSupportedException("Provided constructor not applicable to type.");
            //Create a new object of type T using the provided constructor
            return (T)constructor.Invoke(args);
        }

        public static bool NetworkShareAvailable(string path)
        {
            if (string.IsNullOrEmpty(path)) return false;
            string pathRoot = path;//Path.LocalToUnc(path);
            int index = pathRoot.IndexOf('/', 2);
            int index2 = pathRoot.IndexOf('\\', 2);
            if (index == -1 || (index2 != -1 && index > index2))
                index = index2;
            if(index > -1)
                pathRoot = pathRoot.Remove(index);
            if (string.IsNullOrEmpty(pathRoot)) return false;
            ProcessStartInfo pinfo = new ProcessStartInfo("net", "view \"" + pathRoot + "\"");
            pinfo.CreateNoWindow = true;
            pinfo.RedirectStandardOutput = true;
            pinfo.UseShellExecute = false;
            string output;
            using (Process p = Process.Start(pinfo))
            {
                output = p.StandardOutput.ReadToEnd();
            }
            if (output.Contains("completed successfully"))
                return true;

            return false;
        }

        public static AccessErrorCode FileOrDirAvailable(string path)
        {
            AccessErrorCode errorCode = AccessErrorCode.None;
            Thread t = new Thread(new ThreadStart(delegate()
            {
                //if (NetworkShareAvailable(path)) Doesn't work for local paths that aren't shared on Network :(
                //{
                    if (File.Exists(path) || Directory.Exists(path))
                        errorCode = AccessErrorCode.None;
                    else
                        errorCode = AccessErrorCode.NotFound;
                //}
                //else errorCode = AccessErrorCode.NetworkShareDown;
            }));
            t.Start();
            if (!t.Join(2500)) //5 second timeout
                errorCode = AccessErrorCode.NetworkShareDown;

            return errorCode;
        }
    }

    public enum AccessErrorCode
    {
        None,
        NetworkShareDown,
        NotFound
    }
}
