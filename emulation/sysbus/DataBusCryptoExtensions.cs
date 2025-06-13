using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Security.Cryptography;

namespace EmbedEmul.SystemBus.DataBusExtensions
{
    #region CRYPTO EXTENSIONS
    public static class DataBusCryptoExtensions
    {
        public static ushort GetCRC16(this IDataBus bus, Int64 byteCount, ushort crc = 0x0000)
        {
            return Utilities.CRC16(bus.GetBytes(byteCount), crc);
        }
        public static ulong GetChecksum(this IDataBus bus, Int64 byteCount, ulong checksum = 0, int dataWidth = 2)
        {
            if (dataWidth == 1)
            {
                for (int i = 0; i < byteCount; i++)
                {
                    checksum += bus.GetUInt8();
                }
            }
            else if (dataWidth == 2)
            {
                for (int i = 0; i < byteCount >> 1; i++)
                {
                    checksum += bus.GetUInt16();
                }
            }
            else if (dataWidth == 4)
            {
                for (int i = 0; i < byteCount >> 2; i++)
                {
                    checksum += bus.GetUInt32();
                }
            }
            else if (dataWidth == 8)
            {
                for (int i = 0; i < byteCount >> 3; i++)
                {
                    checksum += bus.GetUInt64();
                }
            }
            else throw new NotImplementedException($"Checksum not implemented for this number of bytes {dataWidth}");

            return checksum;
        }
        public static byte[] GetSHA256(this IDataBus bus, Int64 length)
        {
            byte[] hash;
            using (SHA256 sha256 = SHA256.Create())
            {
                hash = sha256.ComputeHash(bus.GetByteStream(length));
            }
            return hash;
        }
    }
    #endregion
}