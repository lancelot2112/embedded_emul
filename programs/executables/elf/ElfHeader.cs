using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Binary;
using EmbedEmul.Memory;

namespace EmbedEmul.Elf
{
    public enum ElfSpecialSectionIndex : ushort
    {
        SHN_UNDEF = 0x0,
        SHN_LOPROC = 0xff00,
        SHN_HIPROC = 0xff1f,
        SHN_ABS = 0xfff1,
        SHN_COMMON = 0xfff2,
    }

    public enum ElfProcessorClass : byte
    {
        Invalid = 0,
        Address_32Bit = 1,
        Address_64Bit = 2
    }

    public enum ElfType : ushort
    {
        None = 0, //No File Type
        Relocatable = 1, //Relocatable File
        Executable = 2, //Executable File
        Shared = 3, //Shared Object File
        Core = 4 //Core File
    }

    public enum ElfMachine : ushort
    {
        NONE = 0,        //No machine
        M32 = 1,         //AT&T WE 32100
        SPARC = 2,       //SPARC
        Intel_80386 = 3,         //Intel 80386
        Motorola_68K = 4,         //Motorola 68000
        Motorola_88k = 5,         //Motorola 88000
        Intel_80860 = 7,         //Intel 80860
        MIPS = 8,        //MIPS I Architecture
        S370 = 9,        //IBM System/370 Processor
        MIPS_RS3_LE = 10,//MIPS RS3000 Little-endian
        PARISC = 15,     //Hewlett-Packard PA-RISC
        VPP500 = 17,     //Fujitsu VPP500
        SPARC32PLUS = 18,//Enhanced instruction set SPARC
        Intel_80960 = 19,        //Intel 80960
        PowerPC = 20,        //PowerPC
        PowerPC64 = 21,      //64-bit PowerPC
        V800 = 36,       //NEC V800
        FR20 = 37,       //Fujitsu FR20
        RH32 = 38,       //TRW RH-32
        RCE = 39,        //Motorola RCE
        ARM = 40,        //Advanced RISC Machines ARM
        ALPHA = 41,      //Digital Alpha
        SH = 42,         //Hitachi SH
        SPARCV9 = 43,    //SPARC version 9
        TRICORE = 44,    //Siemens Tricore embedded processor
        ARC = 45,        //Argonaut RISC Core, Argonaut Technologies Inc.
        H8_300 = 46,     //Hitachi H8/300
        H8_300H = 47,    //Hitachi H8/300H
        H8S = 48,        //Hitachi H8S
        H8_500 = 49,     //Hitachi H8/500
        IA_64 = 50,      //Intel IA-64 processor architecture
        MIPS_X = 51,     //Stanford MIPS-X
        COLDFIRE = 52,   //Motorola ColdFire
        Motorola_68HC12 = 53,     //Motorola M68HC12
        MMA = 54,        //Fujitsu MMA Multimedia Accelerator
        PCP = 55,        //Siemens PCP
        NCPU = 56,       //Sony nCPU embedded RISC processor
        NDR1 = 57,       //Denso NDR1 microprocessor
        STARCORE = 58,   //Motorola Star*Core processor
        ME16 = 59,       //Toyota ME16 processor
        ST100 = 60,      //STMicroelectronics ST100 processor
        TINYJ = 61,      //Advanced Logic Corp. TinyJ embedded processor family
        FX66 = 66,       //Siemens FX66 microcontroller
        ST9PLUS = 67,    //STMicroelectronics ST9+ 8/16 bit microcontroller
        ST7 = 68,        //STMicroelectronics ST7 8-bit microcontroller
        Motorola_68HC16 = 69,     //Motorola MC68HC16 Microcontroller
        Motorola_68HC11 = 70,     //Motorola MC68HC11 Microcontroller
        Motorola_68HC08 = 71,     //Motorola MC68HC08 Microcontroller
        Motorola_68HC05 = 72,     //Motorola MC68HC05 Microcontroller
        SVX = 73,        //Silicon Graphics SVx
        ST19 = 74,       //STMicroelectronics ST19 8-bit microcontroller
        VAX = 75,        //Digital VAX
        CRIS = 76,       //Axis Communications 32-bit embedded processor
        JAVELIN = 77,    //Infineon Technologies 32-bit embedded processor
        FIREPATH = 78,   //Element 14 64-bit DSP Processor
        ZSP = 79,        //LSI Logic 16-bit DSP Processor
        MMIX = 80,       //Donald Knuth's educational 64-bit processor
        HUANY = 81,      //Harvard University machine-independent object files
        PRISM = 82       //SiTera Prism

    }

   public enum MachEABIFlags : ulong
   {
      EF_TRICORE_V1_1 = (ElfMachine.TRICORE << 32) | 0x80000000,
      EF_TRICORE_V1_2 = (ElfMachine.TRICORE << 32) | 0x40000000,
      EF_TRICORE_V1_3 = (ElfMachine.TRICORE << 32) | 0x20000000, //Adds MMU and related to v1.2
      EF_TRICORE_V1_3_1 = (ElfMachine.TRICORE << 32) | 0x00800000, //Expands intruction set of v1.3
      EF_TRICORE_V1_6 = (ElfMachine.TRICORE << 32) | 0x00400000,  //Extends 1.3.1
      EF_TRICORE_V1_6_1 = (ElfMachine.TRICORE << 32) | 0x00200000, //1.6P/E implemented as AURIX family.. used to be V1_6_PE, extends 1.6
      EF_TRICORE_V1_6_2 = (ElfMachine.TRICORE << 32) | 0x00100000, //extends 1.6.1
      EF_TRICORE_PCP = (ElfMachine.TRICORE << 32) | 0x01000000,
      EF_TRICORE_PCP2 = (ElfMachine.TRICORE << 32) | 0x02000000
   }
    /// <summary>
    /// Holds ELF file information.  REF: http://www.skyfree.org/linux/references/ELF_Format.pdf page 1-3
    /// </summary>
    public class ElfHeader
    {
        internal const int IDENTIFIER_LEN = 16;
        internal const int TOTAL_LEN = 52;
        /// <summary>
        /// The initial bytes mark the file as an object file and provide machine-independent data
        /// with which to decode and interpret the file's contents.
        ///
        /// {0x7F 'E' 'L' 'F' [class] [data] [version] [pad...till end] }
        ///
        /// class:  0 Invalid class
        ///         1 32-bit objects
        ///         2 64-bit objects
        /// data:   0 Invalid data encoding
        ///         1 Little Endian
        ///         2 Big Endian
        /// </summary>
        internal byte[] _identifier;  //unsigned char[IDENT_LEN]

        /// <summary>
        /// Is true if file identifier starts with the bytes {0x7f 'E' 'L' 'F'} indicating file is an elf file
        /// </summary>
        bool IsValidFile
        {
            get
            {
                return _identifier[0] == 0x7f &&
                       _identifier[1] == (byte)'E' &&
                       _identifier[2] == (byte)'L' &&
                       _identifier[3] == (byte)'F';
            }
        }

        public ElfProcessorClass Class
        {
            get { return (ElfProcessorClass)_identifier[4]; }
        }

        public ByteOrder ByteOrder
        {
            get { return (ByteOrder)_identifier[5]; }
        }

        public byte OSABI
        {
            get { return _identifier[7]; }
        }

        public byte ABIVersion
        {
            get { return _identifier[8]; }
        }

        /// <summary>
        /// Identifies object file type.
        /// Value | Name     | Meaning
        /// 0     | ET_NONE  | No file type
        /// 1     | ET_REL   | Relocatable file
        /// 2     | ET_EXEC  | Executable file
        /// 3     | ET_DYN   | Shared object file
        /// 4     | ET_CORE  | Core file
        /// 0xff00| ET_LOPROC| Processor-specific
        /// 0xffff| ET_HIPROC| Processor-specific
        ///
        /// Although core file contents are unspecified, type ET_CORE is reserved to mark the
        /// file. Values from ET_LOPROC through ET_HIPROC (inclusive) are reserved for
        /// processor-specifi semantics. Other values are reserved and will be assigned to new
        /// object file types as necessary.
        /// </summary>
        public ElfType FileType { get { return (ElfType)_type; } }
        internal UInt16 _type; //Elf32_Half


        /// <summary>
        /// This member's value specifies the required architecture for an individual file.
        /// Value | Name    | Meaning
        /// 0     | EM_NONE | No machine
        /// 1     | EM_M32  | AT&T WE 32100
        /// 2     | EM_SPARC| SPARC
        /// 3     | EM_386  | Intel 80386
        /// 4     | EM_68K  | Motorola 68000
        /// 5     | EM_88k  | Motorola 88000
        /// 6     |
        /// 7     | EM_860  | Intel 80860
        /// 8     | EM_MIPS | MIPS RS3000
        ///
        /// Other values are reserved and will be assigned to new machines as necessary.
        /// Processor-specific ELF names use the machine name to distinguish them.  For example,
        /// the flags mentionsed below use the prefix EF_; a flag named WIDGET for the EM_XYZ
        /// machine would be called EF_XYZ_WIDGET
        /// </summary>
        public ElfMachine MachineType { get { return (ElfMachine)_machine; } }
        internal UInt16 _machine; //Elf32_Half


        /// <summary>
        /// Identifies object file version (0 Invalid; 1 Current)... newer versions will be higher numbers.
        /// </summary>
        public UInt32 Version { get { return _version; } }
        internal UInt32 _version; //Elf32_Word

        /// <summary>
        /// This member gives the Virtual address to which the system first transfers control, thus
        /// starting the process. If the file has no associated entry point, this member holds zero.
        /// </summary>
        public UInt32 Entry { get { return _entry; } }
        internal UInt32 _entry; //Elf32_Addr

        /// <summary>
        /// This member holds the program header table's file offset in bytes.  If the file has no
        /// program header table, this member holds zero.
        /// </summary>
        public UInt32 SegmentHeaderOffset { get { return _segmentHeaderOffset; } }
        internal UInt32 _segmentHeaderOffset; //Elf32_Off

        /// <summary>
        /// This member holds the section header table's file offset in bytes.  If the file has no section
        /// header table, this member holds zero.
        /// </summary>
        public UInt32 SectionHeaderOffset { get { return _sectionHeaderOffset; } }
        internal UInt32 _sectionHeaderOffset; //Elf32_Off

        /// <summary>
        /// This member holds processor-specific flags associated with the file.  Flag names take the
        /// form EF_machine_flag.
        /// </summary>
        public MachEABIFlags Flags { get { return (MachEABIFlags)(((ulong)_machine << 32) | _flags); } }
        internal UInt32 _flags; //Elf32_Word

        /// <summary>
        /// This member holds the ELF header's size in bytes.
        /// </summary>
        public UInt16 HeaderSize { get { return _elfHeaderSize; } }
        internal UInt16 _elfHeaderSize; //Elf32_Half

        /// <summary>
        /// This member holds the size in bytes of one entry in the file's program header table; all
        /// entries are the same size.
        /// </summary>
        public UInt16 SegmentHeaderEntrySize { get { return _segmentHeaderEntrySize; } }
        internal UInt16 _segmentHeaderEntrySize; //Elf32_Half

        /// <summary>
        /// This member holds the number of entries in the program header table.  The the product
        /// of _phentsize and _phnum gives the table's size in bytes.  If a file has no program header
        /// table, _phnum holds the value zero.
        /// </summary>
        public UInt32 SegmentHeaderEntryCount { get { return _segmentHeaderEntryCount; } }
        internal UInt32 _segmentHeaderEntryCount; //Elf32_Half

        /// <summary>
        /// This member holds a section header's size in bytes.  A section header is one entry in
        /// the section header table;  al entries are the same size.
        /// </summary>
        public UInt16 SectionHeaderEntrySize { get { return _sectionHeaderEntrySize; } }
        internal UInt16 _sectionHeaderEntrySize; //Elf32_Half


        /// <summary>
        /// This member holds the number of entries in the section header table.  Thus the product
        /// of _shentsize and _shnum gives the sectio nheader table's size in bytes.  If a file
        /// has no section header table, _shnum holds the value zero.
        /// </summary>
        public UInt32 SectionHeaderEntryCount { get { return _sectionHeaderEntryCount; } }
        internal UInt32 _sectionHeaderEntryCount; //Elf32_Half


        /// <summary>
        /// This member holds the section header table index of the entry associated with the section
        /// name string table.  If the file has no section name string table, this member holds
        /// the value SHN_UNDEF.
        /// </summary>
        public UInt16 SectionHeaderNameStringTableIndex { get { return _sectionHeaderNameStringTableIndex; } }
        internal UInt16 _sectionHeaderNameStringTableIndex; //Elf32_Half

        /// <summary>
        /// Most of the time this is 0, however there are certain elf files where the "MAGIC" bytes are not the
        /// first bytes in the file.  There were added bytes with other unrelated information.  By searching for
        /// these magic bytes first we can offset the entire "elf file" contents by this offset to skip
        /// the unrelated information.
        /// </summary>
        public Int64 ElfHeaderOffset { get { return _elfHeaderOffset; } }
        internal Int64 _elfHeaderOffset;

        public ElfHeader(System.IO.BinaryReader stream)
        {
            _elfHeaderOffset = -1;
            bool validHeader = false;
            _identifier = new byte[IDENTIFIER_LEN];
            Int64 streamLen = stream.BaseStream.Length;
            while (!validHeader && stream.BaseStream.Position != streamLen)
            {
                while (_identifier[0] != 0x7f)
                    _identifier[0] = stream.ReadByte();

                _identifier[1] = stream.ReadByte();
                if (_identifier[1] == (byte)'E')
                {
                    _identifier[2] = stream.ReadByte();
                    if (_identifier[2] == (byte)'L')
                    {
                        _identifier[3] = stream.ReadByte();
                        if (_identifier[3] == (byte)'F')
                            validHeader = true;
                        else _identifier[0] = _identifier[3];
                    }
                    else _identifier[0] = _identifier[2];
                }
                else _identifier[0] = _identifier[1];
            }

            if (validHeader)
            {
                //Finds the elf header and skips any unrelated bytes added at the beginning of the file.
                //All "file offsets" will be relative to this position in the file stream.
                _elfHeaderOffset = (stream.BaseStream.Position - 4);
                Array.Copy(stream.ReadBytes(IDENTIFIER_LEN - 4), 0, _identifier, 4, IDENTIFIER_LEN - 4);

                MemoryUnit headerBlock = new MemoryUnit(stream.ReadBytes(TOTAL_LEN - IDENTIFIER_LEN), ByteOrder);
                GetHeader(headerBlock);
            }
        }

        public void GetHeader(MemoryUnit data)
        {
            //Assert that we have enough bytes as a sanity check
            Debug.Assert(data.BufferLength >= TOTAL_LEN - IDENTIFIER_LEN);

            _type = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx, ByteOrder);
            _machine = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 2, ByteOrder);
            _version = data.GetUInt32();// Utilities.BytesToUInt32(buffer, startidx + 4, ByteOrder);
            _entry = data.GetUInt32();// Utilities.BytesToUInt32(buffer, startidx + 8, ByteOrder);
            _segmentHeaderOffset = data.GetUInt32();// Utilities.BytesToUInt32(buffer, startidx + 12, ByteOrder);
            _sectionHeaderOffset = data.GetUInt32();// Utilities.BytesToUInt32(buffer, startidx + 16, ByteOrder);
            _flags = data.GetUInt32();// Utilities.BytesToUInt32(buffer, startidx + 20, ByteOrder);
            _elfHeaderSize = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 24, ByteOrder);
            _segmentHeaderEntrySize = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 26, ByteOrder);
            _segmentHeaderEntryCount = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 28, ByteOrder);
            _sectionHeaderEntrySize = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 30, ByteOrder);
            _sectionHeaderEntryCount = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 32, ByteOrder);
            _sectionHeaderNameStringTableIndex = data.GetUInt16();// Utilities.BytesToUInt16(buffer, startidx + 34, ByteOrder);

        }

        public override string ToString()
        {
            return string.Format(" {0:X2} {1}{2}{3}\n {4}\n {5}\n type: {6}\n machine: {7}\n version: {8:X4}\n" +
                                " entry: {9:X4}\n phoff: {10:X4}\n shoff: {11:X4}\n flags: {12:X4}\n ehsize: {13:X2}\n" +
                                " phesize: {14:X2}\n phecount: {15:X2}\n shesize: {16:X2}\n shecount: {17:X2}\n strtblidx: {18:X2}\n",
                _identifier[0],
                (char)_identifier[1],
                (char)_identifier[2],
                (char)_identifier[3],
                Class,
                ByteOrder,
                FileType,
                MachineType,
                _version,
                _entry,
                _segmentHeaderOffset,
                _sectionHeaderOffset,
                _flags,
                _elfHeaderSize,
                _segmentHeaderEntrySize,
                _segmentHeaderEntryCount,
                _sectionHeaderEntrySize,
                _sectionHeaderEntryCount,
                _sectionHeaderNameStringTableIndex);
        }
    }
}
