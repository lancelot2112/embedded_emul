using System.IO;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Tools;
using EmbedEmul.Memory;


namespace EmbedEmul.Programs.Binary
{
    public class BinaryImage
    {
        /// <summary>
        /// Binary file describes the memory layout of a code file.  Oftentimes type information
        /// can be gathered from a supplementary file. Initially will be interpreted as raw
        /// bytes and can be viewed however one wishes.  Type information can be added manually
        /// for structured access.
        /// </summary>
        internal TypedMemory _memory;
        public TypedMemory Memory { get { return _memory; } }
        public IEnumerable<MemoryUnit> Blocks { get { return _memory._raw.Blocks; } }
        internal bool _hasEntry;
        public bool HasEntry { get { return _hasEntry; } }
        internal UInt64 _entryPoint;
        public UInt64 EntryPoint { get { return _entryPoint; } }
        internal UInt16 _identifier;
        public UInt16 Identifier { get { return _identifier; } }
        internal object _extra;

        internal FileInfo _fileInfo;
        internal TrustLevel _trustLevel;
        public event StatusUpdateDelegate StatusUpdate;

        internal BinaryImage()
        {
            _trustLevel = TrustLevel.Full;
        }
        public BinaryImage(string path, List<MemoryUnit> blocks, UInt32 entry = 0x0, bool hasEntry = false, StatusUpdateDelegate statusHandlers = null)
        {
            if (statusHandlers != null)
                StatusUpdate += statusHandlers;

            _fileInfo = new FileInfo(path);
            _memory = new TypedMemory();
            _memory.LinkBlocks(blocks);
            _entryPoint = entry;
            _hasEntry = hasEntry;
            _trustLevel = TrustLevel.Full;
        }

        public BinaryImage(string path, StatusUpdateDelegate statusHandlers = null)
        {
            if (statusHandlers != null)
                StatusUpdate += statusHandlers;

            _fileInfo = new FileInfo(path);
            _memory = new TypedMemory();
            _trustLevel = TrustLevel.Full;
        }

        public void AddBlocks(IEnumerable<MemoryUnit> blocks)
        {
            _memory.LinkBlocks(blocks);
        }
        public void AddBlock(MemoryUnit block)
        {
            _memory.LinkBlock(block);
        }
        public void SetEntry(UInt64 entryPoint)
        {
            _entryPoint = entryPoint;
            _hasEntry = true;
        }

        /// <summary>
        /// Calculated by taking the CRC of the start and used length for each contiguous block.  Same identifier is calculated for
        /// each elf file, used to speed up lookup and identification in the absense of a compatibility header. Not sure what this is doing...
        /// </summary>
        public IEnumerable<AddressRange> CalculateIdentifier()
        {
            UInt64 addressOfLastUsedByte;
            Int64 usedLength;
            byte[] startBytes;
            byte[] lengthBytes;
            AddressRange currentRange = new AddressRange();
            foreach (MemoryUnit block in Blocks)
            {
                addressOfLastUsedByte = block.ScanToLastUsedByte();
                usedLength = (Int64)(addressOfLastUsedByte - block.Range.Start + 1);

                startBytes = BitConverter.GetBytes(block.Range.Start);
                _identifier = Utilities.CRC16(startBytes, _identifier);
                lengthBytes = BitConverter.GetBytes(usedLength);
                _identifier = Utilities.CRC16(lengthBytes, _identifier);

                currentRange.MoveTo(block.Range.Start);
                currentRange.Resize(usedLength);
                yield return currentRange;
            }
        }

        public static Type GetFileClassType(string filePath)
        {
            string extension = Path.GetExtension(filePath);
            if (MotorolaSRecordFile.VALID_EXTENSIONS.Contains(extension))
                return typeof(MotorolaSRecordFile);
            else if (IntelHexidecimalFile.VALID_EXTENSIONS.Contains(extension))
                return typeof(IntelHexidecimalFile);
            else
                return null;
        }

        /// <summary>
        /// Methods take any initial BinaryFile type and uses the individual methods to open the file
        /// </summary>
        /// <param name="filePath"></param>
        /// <returns></returns>
        public static BinaryImage FromPath(string filePath, StatusUpdateDelegate statusHandlers = null)
        {
            BinaryImage returnFile;
            using(var fileStream = File.OpenRead(filePath))
                returnFile = BinaryImage.FromStream(filePath, fileStream, statusHandlers);
            return returnFile;
        }
        public static BinaryImage FromStream(string filePath, System.IO.Stream stream, StatusUpdateDelegate statusHandlers = null)
        {
            string extension = Path.GetExtension(filePath).ToLower();
            if (IntelHexidecimalFile.VALID_EXTENSIONS.Contains(extension))
                return IntelHexidecimalFile.FromStream(filePath, stream, statusHandlers);
            else if (MotorolaSRecordFile.VALID_EXTENSIONS.Contains(extension))
                return MotorolaSRecordFile.FromStream(filePath, stream, statusHandlers);
            else return null;
        }

        /// <summary>
        /// Methods take original BinaryFile of any type and converts it to any other type.
        /// </summary>
        /// <param name="filePath"></param>
        /// <param name="original"></param>
        public static void ToFile(string filePath, BinaryImage original)
        {
            using(var fileStream = File.Create(filePath))
            {
                BinaryImage.ToFile(filePath, fileStream, original);
            }

        }
        public static void ToFile(string filePath, System.IO.Stream stream, BinaryImage original)
        {
            string extension = Path.GetExtension(filePath).ToLower();
            if (IntelHexidecimalFile.VALID_EXTENSIONS.Contains(extension))
                IntelHexidecimalFile.ToStream(stream, original);
            else if (MotorolaSRecordFile.VALID_EXTENSIONS.Contains(extension))
                MotorolaSRecordFile.ToStream(stream, original);
            else ;//do nothing

            original._memory._raw.ClearEditCounter();
        }

        internal void OnStatusUpdate(object caller, string name, string description, StatusUpdateType type)
        {
            if(StatusUpdate != null)
                StatusUpdate(caller, name, description, type);
        }
    }
}
