using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using GenericUtilitiesLib;
using EmbedEmul.Binary;
using EmbedEmul.Memory;
using EmbedEmul.Variables;

namespace EmbedEmul.Hardware
{
   public enum EmulMode : byte
   {
      bits32,
      bits64
   }
   public enum BranchMode :byte
   {
      None,
      Branch,
      Link
   }
   public abstract class ArchEmulator
   {

      public EmulMode Mode;
      public BranchMode BranchMode;
      public Variable CIVar;
      public UInt64 CIA; //Program counter or Current Instruction Address (CIA)
      public UInt64 NIA; //Next Instruction Address (NIA) calculated
      public Variable LinkVar;
      public UInt64 LinkR; //Linked instruction address
      public UInt64 CountR;


      public ArchOp OP;
      public string OPPostfix;
      public UInt64 OPRaw;
      public UInt64[] OPFs = new UInt64[10]; //Op field values
      public UInt64[] GenPR;
      public UInt64[] SpecPR;
      public UInt64 CondR;


      public abstract ArchOpField[] Fields { get; }
      public abstract ArchOpGroup[] Ops { get; }
      public byte OC_8bit = 0;
      public UInt16 OC_16bit = 0;
      public UInt32 OC_32bit = 0;

      public bool TrySeek(UInt64 address, MemoryManager mem)
      {
         address = address & 0xFFFFFFFFFFFFFFFC; //align
         if (mem.TrySeek(address) == MemoryManagerState.Valid)
            return false;

         CIA = (UInt32)address;

         OP = null;

         OPRaw = (UInt32)mem.GetUnsigned(4);

         foreach(ArchOpGroup group in Ops)
         {
            if((OPRaw & group._opcode_mask) == group._opcode)
            {
               foreach (ArchOp insn in group._insns)
               {
                  if ((OPRaw & group._ext_opcode_mask) == insn._opcode)
                  {
                     OP = insn;
                     break;
                  }
               }

               if (OP != null)
                  break;
            }
         }

         if (OP == null)
         {
            OP = ArchOp.TODO;
            return true;
         }

         OP.DECODE(this);

         return true;
      }

      public string DISPLAY(VariableTable varTab)
      {
         var sb = ObjectFactory.StringBuilders.GetObject();
         sb.Clear();

         if (varTab != null && varTab.TryGetVariableByAddress(CIA, out CIVar))
         {
            if(CIVar._fileAddress == CIA)
            {
               sb.Append(";----START:fcn.");
               sb.Append(CIVar.Label);
               sb.AppendLine();
            }
         }

         sb.AppendFormat("[0x{0:X8}] {1:X8} ", CIA, OPRaw);

         if (OP != null)
            OP.DISPLAY(this, varTab, sb);

         if(CIVar != null && ((CIVar._fileAddress + CIVar._size) == (CIA+4)))
         {
            sb.AppendLine();
            sb.Append(";----END:fcn.");
            sb.Append(CIVar.Label);
         }

         string ret = sb.ToString();
         ObjectFactory.StringBuilders.ReleaseObject(sb);
         return ret;
      }

      /*** Field Type (FT) bit definitions, helps with emulation ***/
      public const UInt32 FT_NOP = 0;
      public const UInt32 FT_Immd = 1 << 1;   //Field containing data of any type
      public const UInt32 FT_Reg = 1 << 7;
      public const UInt32 FT_GEN = 1 << 8;            //Field pointing to a general purpose register
      public const UInt32 FT_FLOAT = 1 << 9;  //Registers for the floating point processor
      public const UInt32 FT_COND = 1 << 10;   //Holds results of comparisons (floating or fixed)
      public const UInt32 FT_SEG = 1 << 11;
      public const UInt32 FT_LINK = 1 << 12;   //Branch targets and holds return address
      public const UInt32 FT_CNT = 1 << 13;    //Loop counts for use in branch instructions
      public const UInt32 FT_IDX = 1 << 14;    //Index registers to compute address from base address (eg. array indexing)
      public const UInt32 FT_INTC = 1 << 15;   //Interrupt Control, turning on and off and differentiating
      public const UInt32 FT_INSN = 1 << 16;   //Bytes for current instruction
      public const UInt32 FT_STACKP = 1 << 17; //Pointer to current location in stack
      public const UInt32 FT_MACHST = 1 << 18; //Machine state register holding current state
      public const UInt32 FT_X = 1 << 19;      //Exception register
      public const UInt32 FT_PC = 1 << 20; //Program Counter, current program location
      public const UInt32 FT_SP = 1 << 21; //Special purpose register
      public const UInt32 FT_Trap = 1 << 3;
      public const UInt32 FT_Hint = 1 << 4;
      public const UInt32 FT_Options = 1 << 5;
      public const UInt32 FT_Mask = 1 << 6;
      public const UInt32 FT_Signed = 1 << 2;
      public const UInt32 FT_Hidden = 1U << 31;

      public const UInt32 IT_FIX = 0;
      public const UInt32 IT_FLT = 0;
      public const UInt32 IT_TFORM = 0;
      public const UInt32 IT_CTRL = 0;
      public const UInt32 IT_MEMSYNC = 0;
      public const UInt32 IT_CACHE = 0;
      public const UInt32 IT_LOAD = 0;
      public const UInt32 IT_STORE = 0;
      public const UInt32 IT_BRANCH = 0;
      public const UInt32 IT_SYS = 0;
      public const UInt32 IT_TRAP = 0;
      public const UInt32 IT_CMP = 0;
      public const UInt32 IT_LOGICAL = 0;
      public const UInt32 IT_ROTSHFT = 0;
      public const UInt32 IT_TLB = 0;
   }
}
