using System.IO;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.Binary
{


    public static class MotorolaSRecordFile
    {
        public static HashSet<string> VALID_EXTENSIONS = new HashSet<string>()
        {
            ".srec",
            ".s19",
            ".s28",
            ".s37",
            ".s1",
            ".s2",
            ".s3"
        };

        //internal string _header;
        //public string Header { get { return _header; } }

        //public MotorolaSRecordFile(string path, StatusUpdateDelegate statusHandlers = null)
        //    : base(path, statusHandlers) { }

        //public MotorolaSRecordFile(List<BinaryBlock> blocks, UInt32 entryPoint, bool hasEntry)
        //    : base(blocks, entryPoint, hasEntry) { }

        public static BinaryImage FromFile(string filePath, StatusUpdateDelegate statusHandlers = null)
        {
            using (var stream = File.OpenRead(filePath))
                return FromStream(filePath, stream, statusHandlers);

        }
        public static BinaryImage FromStream(string filePath, System.IO.Stream stream, StatusUpdateDelegate statusHandlers = null)
        {
            BinaryImage file = new BinaryImage(filePath, statusHandlers);

            if (!VALID_EXTENSIONS.Contains(Path.GetExtension(filePath).ToLower()))
            {
                file._trustLevel = TrustLevel.Error;
                file.OnStatusUpdate(file, Path.GetFileName(filePath), "File does not have MotorolaSRecord file extensions.", StatusUpdateType.Error);
                return file;
            }

            MotorolaSRecord record;
            string line = "";
            UInt64 lineCount = 0, lastMismatchLine = 0;
            byte recordType, recordLen;
            int checksumMismatchCount = 0;
            UInt32 expectedDataRecordCount = 0; //optional
            List<MotorolaSRecord> records = new List<MotorolaSRecord>();
            List<MemoryUnit> blocks = new List<MemoryUnit>();
            using (var reader = new System.IO.StreamReader(stream))
            {
                while (!reader.EndOfStream)
                {
                    line = reader.ReadLine();
                    lineCount++;

                    if (line.Length < 4) continue; //minimum valid SRecord requires at least 10 chars however first 4 denote record type and length

                    if (line[0] == 'S') //valid SRecord starts with an S
                    {
                        //get record type and check for validity... if not as expected indicate an error and stop the read
                        recordType = (byte)line[1];
                        if (recordType >= 0x30 && recordType <= 0x39) //0-9
                        {
                            //get record length and check for matching length of string... if mismatch indicate an error and stop the read
                            recordLen = (byte)
                            (
                            Utilities.NibbleHex2ValueTable[line[2]] << 4 |
                            Utilities.NibbleHex2ValueTable[line[3]]
                            );
                            if ((recordLen << 1) + 4 == line.Length)
                            {
                                record = new MotorolaSRecord(line, (byte)(recordType - 0x30), recordLen);
                                //if record is an S1,S2,S3 add to the list
                                if (record._type >= 1 && record._type <= 3)
                                    records.Add(record);
                                else if (record._type == 0)
                                    file._extra = new string(record._data.Select(b => (char)b).ToArray());
                                else if (record._type == 5 || record._type == 6)//record indicates expected number of entries
                                    expectedDataRecordCount = (UInt32)record._startAddress;
                                else //record is of type S7, S8, S9 which terminates the file and indicates the entry point
                                {
                                    file.SetEntry(record._startAddress);
                                }

                                if (record._checksum != record.CalculateChecksum())
                                {
                                    checksumMismatchCount++;
                                    lastMismatchLine = lineCount;
                                }
                            }
                            else //Malformed data record... supplied text was longer or shorter than expected
                            {
                                file._trustLevel = TrustLevel.Error;
                                file.OnStatusUpdate(file, Path.GetFileName(filePath), string.Format("Record length {0} does not match line length, {1} - cannot continue. line:{2}.", (recordLen << 1) + 4, line.Length, lineCount), StatusUpdateType.Error);
                                return file;
                            }
                        }
                        else //SRecord format only defines S1-S9... any others are not valid.
                        {
                            file._trustLevel = TrustLevel.Error;
                            file.OnStatusUpdate(file, Path.GetFileName(filePath), string.Format("Incorrect SRecord type {0}, cannot continue. line:{1}.", recordType, lineCount), StatusUpdateType.Error);
                            return file;
                        }
                    }
                }

                //Check to see if there were any checksum mismatches
                if (checksumMismatchCount > 0)
                {
                    file._trustLevel = TrustLevel.Warning;
                    file.OnStatusUpdate(file, Path.GetFileName(filePath), string.Format("{0} incorrect record checksums. Data integrity may be violated. line:{1}", checksumMismatchCount, lastMismatchLine), StatusUpdateType.Warning);
                }

                //Verify that the expected number of data records were encountered
                if (expectedDataRecordCount > 0 && expectedDataRecordCount != records.Count)
                {
                    file._trustLevel = TrustLevel.Warning;
                    file.OnStatusUpdate(file, Path.GetFileName(filePath), string.Format("Expected {0} records... found {1}. Data integrity may be violated.", expectedDataRecordCount, records.Count), StatusUpdateType.Warning);
                }

                //If we got this far then we have not encountered an error... order the data records by address and create blocks
                records.Sort((r1, r2) => r1._startAddress.CompareTo(r2._startAddress));
                AddressRange blockAddress = new AddressRange();

                UInt64 expectedAddress = records[0]._startAddress;
                blockAddress._start = expectedAddress;

                //perform one pass to create the blocks
                foreach (MotorolaSRecord rec in records)
                {
                    if (rec._startAddress != expectedAddress)
                    {
                        //finish out current block
                        blockAddress._length = (uint)(expectedAddress - blockAddress._start);
                        blocks.Add(new MemoryUnit(ByteOrder.Invalid, blockAddress));
                        //initialize values for next block
                        blockAddress._start = rec._startAddress;
                    }

                    expectedAddress = rec._startAddress + (UInt64)rec._data.Length;
                }
                //add the last block if the Address does not equal the last blocks address
                if (blocks.Count == 0 || !blocks[blocks.Count - 1]._range._start.Equals(blockAddress))
                {
                    MotorolaSRecord lastrec = records[records.Count - 1];
                    blockAddress._length = (uint)(lastrec._startAddress + (UInt64)lastrec._data.Length - blockAddress._start);
                    blocks.Add(new MemoryUnit(ByteOrder.Invalid, blockAddress));
                }

                //perform second pass to copy data into the blocks
                int blockIndex = 0;
                int index = 0;
                foreach (MotorolaSRecord rec in records)
                {
                    if (blocks[blockIndex].EndOfStream)
                        blockIndex++;

                    blocks[blockIndex].SetBytes(rec._data, 0, rec._data.Length);
                    index++;
                }
            }

            file.AddBlocks(blocks);
            file.CalculateIdentifier();

            return file;
        }

        public static void ToFile(string filePath, BinaryImage file)
        {
            string tempPath = Utilities.GetTempFileName(filePath);
            using (var fileStream = File.Create(tempPath))
                MotorolaSRecordFile.ToStream(fileStream, file);

            File.Copy(tempPath, filePath, true);
            File.Delete(tempPath);
        }

        public static void ToStream(System.IO.Stream stream, BinaryImage file)
        {
            using (var writer = new System.IO.StreamWriter(stream))
            {
                writer.NewLine = "\n";

                //create buffer record and initialize with S0 record values (_type = 0, _startAddress = 0x0000)
                MotorolaSRecord record = new MotorolaSRecord();
                record._data = ("Cummins Copyright" + DateTime.Now.Date.ToString()).Select(c => (byte)c).ToArray();
                record._length = (byte)(record._data.Length + 3); //package length + 2 byte address + 1 byte checksum
                writer.WriteLine(record.Finalize());

                //initialize record._data with 32 byte array
                record._data = new byte[32];
                UInt64 finalAddress;
                long currIndex, length;
                UInt64 count = 0;
                foreach (MemoryUnit block in file.Blocks.OrderBy(b=>b._range._start))
                {
                    record._startAddress = block._range._start;
                    record._type = 3;
                    //get the address of the byte just after the end of the block
                    finalAddress = block._range._start + (UInt64)block._range._length;
                    while (record._startAddress < finalAddress)
                    {
                        currIndex = (long)(record._startAddress - block._range._start); //current index into the block
                        length = (long)block._range._length - currIndex; //length of block remaining to be written
                        length = 32 < length ? 32 : length; //get the minimum of the remaining length and "maximum" record length of 32
                        //TODO: Could use the block data byte array directly instead of temporarily copying into a buffer to create the records and save execution cycles
                        Array.Copy(block._data, currIndex, record._data, 0, length);
                        record._length = (byte)(length + 5); //package length + 4 byte address + 1 byte checksum
                        //write the S3 record created above to the file
                        writer.WriteLine(record.Finalize());
                        count++;

                        //get the address of the next record;
                        record._startAddress += (UInt64)length;
                    }
                }

                //now to terminate we first write the S5 (or S6 if count is > 65535) record to indicate how many S3 records were written
                //the record count is in the same place as the start address of the S3 records
                record._startAddress = count;
                if (count <= 65535) //choose between S5 and S6 records
                {
                    record._type = (byte)5;
                    record._length = 3;
                }
                else
                {
                    record._type = (byte)6;
                    record._length = 4;
                }
                writer.WriteLine(record.Finalize());

                //Finally write the termination S7 record with the program entry point (S7 because we used 32-bit addresses in the S3 record)
                record._startAddress = file._hasEntry ? file._entryPoint : 0;
                record._type = 7;
                record._length = 5;
                writer.WriteLine(record.Finalize());

                writer.Flush();
            }

        }

        internal class MotorolaSRecord
        {
            internal byte _type;
            internal byte _length;
            internal UInt64 _startAddress;
            internal byte[] _data;
            internal byte _checksum;

            internal MotorolaSRecord() { }
            public MotorolaSRecord(string line, byte recordType, byte length)
            {
                _type = recordType;
                _length = length;

                int len;
                int start;

                //TODO: Update to handle 64-bit address
                if (_type == 3 || _type == 7) //32-bit address
                {
                    _startAddress = (uint)
                    (
                    Utilities.NibbleHex2ValueTable[line[4]] << 28 |
                    Utilities.NibbleHex2ValueTable[line[5]] << 24 |
                    Utilities.NibbleHex2ValueTable[line[6]] << 20 |
                    Utilities.NibbleHex2ValueTable[line[7]] << 16 |
                    Utilities.NibbleHex2ValueTable[line[8]] << 12 |
                    Utilities.NibbleHex2ValueTable[line[9]] << 8 |
                    Utilities.NibbleHex2ValueTable[line[10]] << 4 |
                    Utilities.NibbleHex2ValueTable[line[11]]
                    );
                    len = (_length - 5); //subtract off address bytes + checksum byte
                    start = 12;
                }
                else if (_type == 2 || _type == 6 || (_type == 5 && _length == 4)) //24-bit address, seems some formats don't exactly follow the spec for S5 and S6 records
                {
                    _startAddress = (uint)
                    (
                    Utilities.NibbleHex2ValueTable[line[4]] << 20 |
                    Utilities.NibbleHex2ValueTable[line[5]] << 16 |
                    Utilities.NibbleHex2ValueTable[line[6]] << 12 |
                    Utilities.NibbleHex2ValueTable[line[7]] << 8 |
                    Utilities.NibbleHex2ValueTable[line[8]] << 4 |
                    Utilities.NibbleHex2ValueTable[line[9]]
                    );
                    len = (_length - 4); //subtract off address bytes + checksum byte
                    start = 10;
                }
                else //16-bit address
                {
                    _startAddress = (uint)
                    (
                    Utilities.NibbleHex2ValueTable[line[4]] << 12 |
                    Utilities.NibbleHex2ValueTable[line[5]] << 8 |
                    Utilities.NibbleHex2ValueTable[line[6]] << 4 |
                    Utilities.NibbleHex2ValueTable[line[7]]
                    );
                    len = (_length - 3); //subtract off address bytes + checksum byte
                    start = 8;
                }

                _data = new byte[len];
                for (int ii = 0; ii < len; ii++)
                    _data[ii] = (byte)
                    (
                    Utilities.NibbleHex2ValueTable[line[start + (ii << 1)]] << 4 |
                    Utilities.NibbleHex2ValueTable[line[start + 1 + (ii << 1)]]
                    );
                start += (byte)(len << 1);


                _checksum = (byte)
                (
                Utilities.NibbleHex2ValueTable[line[start]] << 4 |
                Utilities.NibbleHex2ValueTable[line[start + 1]]
                );
            }

            /// <summary>
            /// Builds the string representation of the data packet
            /// </summary>
            /// <returns>SRecord string</returns>
            public string Finalize()
            {
                //calculate the checksum for the current package
                _checksum = CalculateChecksum();
                char[] line = new char[(_length << 1) + 4];
                //write the type
                line[0] = 'S';
                line[1] = Utilities.Nibble2HexTable[_type];

                //write the length
                line[2] = Utilities.Nibble2HexTable[((_length & 0xf0) >> 4)];
                line[3] = Utilities.Nibble2HexTable[((_length & 0x0f))];

                //write the address according to the type (32bit/24bit/16bit)
                int len;
                int start;
                if (_type == 3 || _type == 7) //32-bit address
                {
                    line[4] = Utilities.Nibble2HexTable[((_startAddress & 0xf0000000) >> 28)];
                    line[5] = Utilities.Nibble2HexTable[((_startAddress & 0x0f000000) >> 24)];
                    line[6] = Utilities.Nibble2HexTable[((_startAddress & 0x00f00000) >> 20)];
                    line[7] = Utilities.Nibble2HexTable[((_startAddress & 0x000f0000) >> 16)];
                    line[8] = Utilities.Nibble2HexTable[((_startAddress & 0x0000f000) >> 12)];
                    line[9] = Utilities.Nibble2HexTable[((_startAddress & 0x00000f00) >> 8)];
                    line[10] = Utilities.Nibble2HexTable[((_startAddress & 0x000000f0) >> 4)];
                    line[11] = Utilities.Nibble2HexTable[(_startAddress & 0x0000000f)];
                    len = (_length - 5);
                    start = 12;
                }
                else if (_type == 2 || _type == 6) //24-bit address
                {
                    line[4] = Utilities.Nibble2HexTable[((_startAddress & 0xf00000) >> 20)];
                    line[5] = Utilities.Nibble2HexTable[((_startAddress & 0x0f0000) >> 16)];
                    line[6] = Utilities.Nibble2HexTable[((_startAddress & 0x00f000) >> 12)];
                    line[7] = Utilities.Nibble2HexTable[((_startAddress & 0x000f00) >> 8)];
                    line[8] = Utilities.Nibble2HexTable[((_startAddress & 0x0000f0) >> 4)];
                    line[9] = Utilities.Nibble2HexTable[(_startAddress & 0x00000f)];
                    len = (_length - 4);
                    start = 10;
                }
                else //16-bit address
                {
                    line[4] = Utilities.Nibble2HexTable[((_startAddress & 0xf000) >> 12)];
                    line[5] = Utilities.Nibble2HexTable[((_startAddress & 0x0f00) >> 8)];
                    line[6] = Utilities.Nibble2HexTable[((_startAddress & 0x00f0) >> 4)];
                    line[7] = Utilities.Nibble2HexTable[(_startAddress & 0x000f)];
                    len = (_length - 3);
                    start = 8;
                }

                int index;
                for (byte ii = 0; ii < len; ii++)
                {
                    index = start + (ii << 1);
                    line[index] = Utilities.Nibble2HexTable[((_data[ii] & 0xf0) >> 4)];
                    line[index + 1] = Utilities.Nibble2HexTable[(_data[ii] & 0x0f)];
                }

                start += (byte)(len << 1);
                line[start] = Utilities.Nibble2HexTable[((_checksum & 0xf0) >> 4)];
                line[start + 1] = Utilities.Nibble2HexTable[(_checksum & 0x0f)];

                return new string(line);
            }

            public byte CalculateChecksum()
            {
                byte checksum = _length;
                int dataLen;
                if (_type == 3 || _type == 7) //32-bit address
                {
                    checksum += (byte)(_startAddress >> 24);
                    checksum += (byte)(_startAddress >> 16);
                    checksum += (byte)(_startAddress >> 8);
                    checksum += (byte)(_startAddress);
                    dataLen = (_length - 5);
                }
                else if (_type == 2 || _type == 6 || (_type == 5 && _length == 4)) //24-bit address
                {
                    checksum += (byte)(_startAddress >> 16);
                    checksum += (byte)(_startAddress >> 8);
                    checksum += (byte)(_startAddress);
                    dataLen = (_length - 4);
                }
                else //16-bit address
                {
                    checksum += (byte)(_startAddress >> 8);
                    checksum += (byte)(_startAddress);
                    dataLen = (_length - 3);
                }

                for (int ii = 0; ii < dataLen; ii++ )
                    checksum += _data[ii];

                return (byte)~checksum;

            }

            public override string ToString()
            {
                return new string(_data.Select(b=>(char)b).ToArray());
            }
        }
    }
}
