using System.IO;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Memory;
using EmbedEmul.SystemBus;
using EmbedEmul.Hardware;
using System.Diagnostics;

namespace EmbedEmul.Programs.Binary
{
    public enum IntelHexidecimalRecordType
    {
        Data,
        EndOfFile,
        ExtendedSegmentAddress,
        StartSegmentAddress,
        ExtendedLinearAddress,
        StartLinearAddress
    }

    public static class IntelHexidecimalFile
    {
        public static HashSet<string> VALID_EXTENSIONS = new HashSet<string>()
        {
            ".xcal",
            ".hex"
        };

        //public IntelHexidecimalFile(string path, List<BinaryData> blocks, UInt32 entryPoint = 0x0, bool hasEntry = false, StatusUpdateDelegate statusHandlers = null)
        //    : base(path, blocks, entryPoint, hasEntry, statusHandlers)
        //{  }

        //public IntelHexidecimalFile(string path, StatusUpdateDelegate statusHandlers = null)
        //    : base(path, statusHandlers)
        //{  }

        public static BinaryImage FromStreamReader(string filePath, System.IO.StreamReader reader, StatusUpdateDelegate statusHandlers = null)
        {
            BinaryImage file = new BinaryImage(filePath, statusHandlers);
            string extension = Path.GetExtension(filePath).ToLower();
            if (!VALID_EXTENSIONS.Contains(extension))
            {
                file._trustLevel = TrustLevel.Error;
                file.OnStatusUpdate(file, file._fileInfo.Name, "File does not have IntelHexidecimal file extension.", StatusUpdateType.Error);
                return file;
            }

            //Assign the file path variables
            string fileName = file._fileInfo.Name;

            List<IntelHexidecimalRow> records = new List<IntelHexidecimalRow>();
            List<MemoryUnit> blocks = new List<MemoryUnit>();
            bool firstEntry = true;
            AddressRange currBlockAddress = new AddressRange();
            UInt64 expectedNextAddress = UInt64.MaxValue;
            int currBlockNumber = 0;
            string line;

            int checksumWarningCount = 0;
            UInt32 lastLineWithBadChecksum = 0;
            byte lastLineCalculatedChecksum = 0;
            ByteOrder order = ByteOrder.Invalid;

            UInt32 lineCount = 0;
            line = reader.ReadLine();

            /* REF: .xcal files follow the Intel Hexadecimal Object File format
                * Basic line structure is as follows
                * <Record Mark ':' 1byte><Load Reclen '00-FF' 1byte><Offset '0000-FFFF' 2byte>
                * <Record Type '00-05' 1byte><Data n-bytes (defined in Load Reclen)><Checksum '00-FF' 1byte>
                *
                * Record Types defined as follows
                * '00' Data Record
                * '01' End of File Record
                * '02' Extended Segment Address Record
                * '03' Start Segment Address Record
                * '04' Extended Linear Address Record
                * '05' Start Linear Address Record
                *
                * each differing only in the data payload
                * */
            //Seed initial address of 0 for malformed XCal files that don't include first exteneded address
            //record
            IntelHexidecimalRow currExtAddress = new IntelHexidecimalRow()
            {
                _checksum = 250,
                _data = null,
                _length = 2,
                _offset = 0,
                _startAddress = 0,
                _type = IntelHexidecimalRecordType.ExtendedLinearAddress
            };

            int num = 0;
            int lineLen = 0;
            List<AddressRange> blockRanges = new List<AddressRange>();
            while (!reader.EndOfStream)
            {
                if (lineCount > 0)
                    line = reader.ReadLine();
                lineCount++;
                lineLen = line.Length;
                if (lineLen >= 11 && line[0] == ':')
                {
                    IntelHexidecimalRow record = new IntelHexidecimalRow();

                    record._length = (byte)
                        (
                        Utilities.NibbleHex2ValueTable[line[1]] << 4 |
                        Utilities.NibbleHex2ValueTable[line[2]]
                        );
                    record._offset = (ushort)
                        (
                        Utilities.NibbleHex2ValueTable[line[3]] << 12 |
                        Utilities.NibbleHex2ValueTable[line[4]] << 8 |
                        Utilities.NibbleHex2ValueTable[line[5]] << 4 |
                        Utilities.NibbleHex2ValueTable[line[6]]
                        );
                    record._type = (IntelHexidecimalRecordType)
                        (
                        Utilities.NibbleHex2ValueTable[line[7]] << 4 |
                        Utilities.NibbleHex2ValueTable[line[8]]
                        );
                    num = record._length << 1; //multiply by 2
                    if (lineLen != num + 11)
                    {
                        file._trustLevel = TrustLevel.Error;
                        file.OnStatusUpdate(file, "Read", string.Format("{0}: Unexpected file format. line:{1}", line, lineCount), StatusUpdateType.Error);
                        return file;
                    }

                    record._checksum = (byte)
                        (
                        Utilities.NibbleHex2ValueTable[line[9 + num]] << 4 |
                        Utilities.NibbleHex2ValueTable[line[10 + num]]
                        );

                    //Debug.Assert(record._checksum == record.CalculateChecksum(), "Checksums do not match");
                    if (record._type == IntelHexidecimalRecordType.Data)
                    {
                        record._data = new byte[record._length];
                        for (int ii = 0; ii < record._length; ii++)
                            record._data[ii] = (byte)
                                (
                                Utilities.NibbleHex2ValueTable[line[9 + (ii << 1)]] << 4 |
                                Utilities.NibbleHex2ValueTable[line[10 + (ii << 1)]]
                                );

                        record._startAddress = currExtAddress._startAddress + record._offset;

                        //determine whether we're in a new block
                        if (record._startAddress != expectedNextAddress)
                        {
                            if (!firstEntry)
                            {
                                currBlockAddress._length = (Int64)(expectedNextAddress - currBlockAddress._start);
                                blockRanges.Add(currBlockAddress);
                                blocks.Add(new MemoryUnit(order, currBlockAddress));
                                currBlockNumber++;
                            }
                            else
                            {
                                firstEntry = false;
                                blocks = new List<MemoryUnit>();
                            }

                            currBlockAddress = new AddressRange(record._startAddress);
                        }
                        expectedNextAddress = record._startAddress + record._length;
                        records.Add(record);
                        //_dataRecordMap.Add(record._startAddress, record);
                    }
                    else if (record._type == IntelHexidecimalRecordType.ExtendedLinearAddress)
                    {
                        //length == 2
                        if (record._length == 2)
                        {
                            record._startAddress = (uint)
                                (
                                Utilities.NibbleHex2ValueTable[line[9]] << 28 |
                                Utilities.NibbleHex2ValueTable[line[10]] << 24 |
                                Utilities.NibbleHex2ValueTable[line[11]] << 20 |
                                Utilities.NibbleHex2ValueTable[line[12]] << 16
                                );
                        }
                        else
                        {
                            file._trustLevel = TrustLevel.Error;
                            file.OnStatusUpdate(file, "Read", string.Format("{0}: Unexpected extended linear address data length (not length 2). line:{1}", fileName, lineCount), StatusUpdateType.Error);
                            return file;
                        }

                        currExtAddress = record;
                        records.Add(record);
                    }
                    else if (record._type == IntelHexidecimalRecordType.StartLinearAddress ||
                             record._type == IntelHexidecimalRecordType.StartSegmentAddress)
                    {
                        if (record._length == 4)
                        {
                            record._startAddress = (uint)
                                (
                                Utilities.NibbleHex2ValueTable[line[9]] << 28 |
                                Utilities.NibbleHex2ValueTable[line[10]] << 24 |
                                Utilities.NibbleHex2ValueTable[line[11]] << 20 |
                                Utilities.NibbleHex2ValueTable[line[12]] << 16 |
                                Utilities.NibbleHex2ValueTable[line[13]] << 12 |
                                Utilities.NibbleHex2ValueTable[line[14]] << 8 |
                                Utilities.NibbleHex2ValueTable[line[15]] << 4 |
                                Utilities.NibbleHex2ValueTable[line[16]]
                                );
                        }
                        else
                        {
                            file._trustLevel = TrustLevel.Error;
                            file.OnStatusUpdate(file, "Read", string.Format("{0}: Unexpected start linear/segment address does length (not length 4). line:{1}", fileName, lineCount), StatusUpdateType.Error);
                            return file;
                        }
                        records.Add(record); //Entry point of program

                        file.SetEntry(record._startAddress);
                    }
                    else if (record._type == IntelHexidecimalRecordType.ExtendedSegmentAddress)
                    {
                        if (record._length == 2)
                        {
                            record._startAddress = (uint)
                                (
                                Utilities.NibbleHex2ValueTable[line[9]] << 16 |
                                Utilities.NibbleHex2ValueTable[line[10]] << 12 |
                                Utilities.NibbleHex2ValueTable[line[11]] << 8 |
                                Utilities.NibbleHex2ValueTable[line[12]] << 4
                                );
                        }
                        else
                        {
                            file._trustLevel = TrustLevel.Error;
                            file.OnStatusUpdate(file, "Read", string.Format("{0}: Unexpected extended segment address length. line:{1}", fileName, lineCount), StatusUpdateType.Error);
                            return file;
                        }

                        currExtAddress = record;
                        records.Add(record);
                    }
                    else { } //do nothing

                    byte check = record.CalculateChecksum();
                    if (record._checksum != check)
                    {
                        checksumWarningCount++;
                        lastLineCalculatedChecksum = check;
                        lastLineWithBadChecksum = lineCount;
                    }

                }
            }

            if (checksumWarningCount > 0)
            {
                file._trustLevel = TrustLevel.Warning;
                file.OnStatusUpdate(file, "Read", string.Format("{0}: {1} data records have invalid checksum(s). lastline: {2} calc: {3:X2}", fileName, checksumWarningCount, lastLineWithBadChecksum, lastLineCalculatedChecksum), StatusUpdateType.Warning);
            }

            if (expectedNextAddress != UInt64.MaxValue)
            {
                if (MachineFactory.BestMatch(blockRanges, out Processor proc))
                {
                    Debug.WriteLine($"Processor {proc.GetType().Name} matched for file {fileName}.");
                }
                else Debug.WriteLine($"No processor matched for file {fileName}.");

                //AFTER STREAM READ
                //add the last block
                currBlockAddress._length = (Int64)(expectedNextAddress - currBlockAddress._start);
                blocks.Add(new MemoryUnit(order, currBlockAddress));

                //Convert the index hexidecimal format records into xcal data blocks
                int currBlock = 0;
                foreach (IntelHexidecimalRow data in records)
                {
                    if (data._type == IntelHexidecimalRecordType.Data)
                    {
                        if (!blocks[currBlock].SeekIfContains(data._startAddress))
                            currBlock++;

                        blocks[currBlock].SetBytes(data._data, 0, data._length);
                    }
                }

                file.AddBlocks(blocks);
                file.CalculateIdentifier();
            }
            else
            {
                file._trustLevel = TrustLevel.Error;
                file.OnStatusUpdate(file, "Read", string.Format("Invalid calibration file: {0}.", fileName), StatusUpdateType.Error);
            }

            return file;

        }
        public static BinaryImage FromStream(string filePath, System.IO.Stream stream, StatusUpdateDelegate statusHandlers = null)
        {
            BinaryImage file = null;
            using (var reader = new System.IO.StreamReader(stream))
            {
                file = IntelHexidecimalFile.FromStreamReader(filePath, reader, statusHandlers);
            }
            return file;
        }

        public static BinaryImage FromFile(string filePath, StatusUpdateDelegate statusHandlers = null)
        {
            //using (XCalCRCFileStream file = new XCalCRCFileStream(_tempFilePathOrig))
            using (var fileStream = File.OpenRead(filePath))
                return IntelHexidecimalFile.FromStream(filePath, fileStream, statusHandlers);

                //hasHeader?new XCalCRCFileStream(filePath):new FileStream(filePath,FileMode.Open,FileAccess.Read))

        }

        public static void ToFile(string filePath, BinaryImage file)
        {
            using (var fileStream = File.Create(filePath))
                ToStream(fileStream, file);
        }

        public static void ToStream(System.IO.Stream stream, BinaryImage file)
        {
            using (var writer = new System.IO.StreamWriter(stream))
            {
                writer.NewLine = "\n";

                //create data record rows in Intel Hexidecimal Format from block data
                //Initialize data
                const int width = 0x20;
                IntelHexidecimalRow linearExtendedAddress = new IntelHexidecimalRow();
                linearExtendedAddress._type = IntelHexidecimalRecordType.ExtendedLinearAddress;
                linearExtendedAddress._length = 2;
                IntelHexidecimalRow record = new IntelHexidecimalRow();
                record._type = IntelHexidecimalRecordType.Data;
                record._data = new byte[width];
                long copied = 0; //NOTE: have to use long instead of int because addresses are uints
                long numToCopy;
                UInt64 prevAddress = UInt64.MaxValue;
                //Loop over all blocks
                foreach (MemoryUnit block in file.Blocks.OrderBy(b => b._range._start))
                {
                    while (copied < (long)block._range._length)
                    {
                        //Output an extended linear address record
                        linearExtendedAddress._startAddress = block._range._start + (UInt64)copied;
                        if ((linearExtendedAddress._startAddress & 0xffff0000) != prevAddress)
                            writer.WriteLine(linearExtendedAddress.Finalize());
                        prevAddress = linearExtendedAddress._startAddress & 0xffff0000;
                        //initialize record offset to a masked value of the current extended address
                        record._offset = (ushort)(linearExtendedAddress._startAddress & 0xffff);
                        while (copied < (long)block._range._length)
                        {
                            //copy bytes from block structure into record
                            numToCopy = block._data.LongLength - copied;
                            if (numToCopy > width) numToCopy = width; //limit to max row width
                            if (numToCopy > 0x10000 - record._offset) numToCopy = 0x10000 - record._offset; //prevent overflow of ushort offset
                            Array.Copy(block._data, copied, record._data, 0, numToCopy);
                            copied += numToCopy;
                            record._length = (byte)numToCopy;
                            //output new record
                            writer.WriteLine(record.Finalize());
                            //Increment offset for next record
                            record._offset += (UInt16)numToCopy;
                            //check to see if we rolled over
                            if (record._offset == 0)
                                break;
                        }
                    }

                    //reset copied because we're moving to a new block
                    copied = 0;
                }
                //Program Entry
                if (file._hasEntry)
                {
                    IntelHexidecimalRow entry = new IntelHexidecimalRow();
                    entry._length = 4;
                    entry._startAddress = file._entryPoint;
                    entry._offset = 0;
                    entry._type = IntelHexidecimalRecordType.StartLinearAddress;
                    writer.WriteLine(entry.Finalize());
                }
                //EOF

                writer.Write(":00000001FF");

                writer.Flush(); //ensure stream is flushed
            }
        }





    }

    public class IntelHexidecimalRow
    {
        internal IntelHexidecimalRecordType _type;
        internal UInt64 _startAddress;

        internal byte _length;
        internal ushort _offset;
        internal byte[] _data; //Only used for data type records
        internal byte _checksum;

        internal string Finalize()
        {
            _checksum = CalculateChecksum();

            StringBuilder build = new StringBuilder();
            build.Append(string.Format(":{0}{1}{2}{3}",
                Utilities.Byte2HexTable[_length],
                Utilities.Byte2HexTable[(_offset & 0xFF00) >> 8],
                Utilities.Byte2HexTable[_offset & 0xFF],
                Utilities.Byte2HexTable[(byte)_type]));

            //TODO: Update to support 64-bit addressing
            if (_type == IntelHexidecimalRecordType.Data)
            {
                for (int ii = 0; ii < _length; ii++)
                    build.Append(Utilities.Byte2HexTable[_data[ii]]);
            }
            else if (_type == IntelHexidecimalRecordType.ExtendedLinearAddress)
            {
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF000000) >> 24)]);
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF0000) >> 16)]);
            }
            else if (_type == IntelHexidecimalRecordType.StartLinearAddress)
            {
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF000000) >> 24)]);
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF0000) >> 16)]);
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF00) >> 8)]);
                build.Append(Utilities.Byte2HexTable[(byte)(_startAddress & 0xFF)]);
            }
            else if (_type == IntelHexidecimalRecordType.ExtendedSegmentAddress)
            {
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF000000) >> 24)]);
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF0000) >> 16)]);
            }
            else if(_type == IntelHexidecimalRecordType.StartSegmentAddress)
            {
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF000000) >> 24)]);
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF0000) >> 16)]);
                build.Append(Utilities.Byte2HexTable[(byte)((_startAddress & 0xFF00) >> 8)]);
                build.Append(Utilities.Byte2HexTable[(byte)(_startAddress & 0xFF)]);
                //throw new NotImplementedException("Intel Hexidecimal segment address spec not implemented.");
            }
            else //Do nothing for EOF entry
            {
            }
            build.Append(Utilities.Byte2HexTable[_checksum]);
            return build.ToString();
        }

        internal byte CalculateChecksum()
        {
            byte sum = _length;
            sum += (byte)((_offset & 0xFF00) >> 8);
            sum += (byte)(_offset & 0xFF);
            sum += (byte)_type;
            if (_type == IntelHexidecimalRecordType.Data)
            {
                for (int ii = 0; ii < _length; ii++)
                    sum += _data[ii];
            }
            else if (_type == IntelHexidecimalRecordType.ExtendedLinearAddress)
            {
                sum += (byte)((_startAddress & 0xFF000000) >> 24);
                sum += (byte)((_startAddress & 0xFF0000) >> 16);
            }
            else if (_type == IntelHexidecimalRecordType.StartLinearAddress)
            {
                sum += (byte)((_startAddress & 0xFF000000) >> 24);
                sum += (byte)((_startAddress & 0xFF0000) >> 16);
                sum += (byte)((_startAddress & 0xFF00) >> 8);
                sum += (byte)(_startAddress & 0xFF);
            }
            else if (_type == IntelHexidecimalRecordType.ExtendedSegmentAddress)
            {
                sum += (byte)((_startAddress & 0xFF000000) >> 24);
                sum += (byte)((_startAddress & 0xFF0000) >> 16);
                //throw new NotImplementedException("Intel Hexidecimal segment address spec not implemented.");
            }
            else if (_type == IntelHexidecimalRecordType.StartSegmentAddress)
            {
                sum += (byte)((_startAddress & 0xFF000000) >> 24);
                sum += (byte)((_startAddress & 0xFF0000) >> 16);
                sum += (byte)((_startAddress & 0xFF00) >> 8);
                sum += (byte)(_startAddress & 0xFF);
            }
            else //Do nothing for EOF entry
            {
            }

            return (byte)-sum;
        }

        public override string ToString()
        {
            StringBuilder build = new StringBuilder();
            build.Append(' ');
            build.Append(_type);
            build.Append(' ');
            build.Append(_startAddress.ToString("X8"));
            build.Append(": -len ");
            build.Append(_length);
            build.Append(" -off ");
            build.Append(_offset.ToString("X4"));
            build.Append(" -data ");
            for (int ii = 0; ii < _length; ii++)
                build.AppendFormat("{0:X2}", _data[ii]);
            build.Append(" -chksum ");
            build.Append(_checksum.ToString("X2"));
            return build.ToString();
        }
    }
}
