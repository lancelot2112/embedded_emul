using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using GenericUtilitiesLib;
using EmbedEmul.Variables;

namespace EmbedEmul.Hardware
{
   public class ArchOpField
   {
      public string _label;
      public string _postfix; //Will append to insn name if value is != 0 and postfix != null
      public UInt32 _bits;
      public byte _start;
      public byte _padbits;
      public UInt32 _type;

      /// <summary>
      /// Helper function to create an ArchOpField more in line with the documentation provided by various ABI vendors.
      /// Function bit indexing has 0..31 with 0x80000000 being bit 0
      /// </summary>
      /// <param name="label"></param>
      /// <param name="start"></param>
      /// <param name="end"></param>
      /// <param name="insn_size"></param>
      /// <param name="type"></param>
      /// <param name="postfix"></param>
      /// <returns></returns>
      public static ArchOpField Create(string label, byte start, byte inclusive_end, byte insn_size, UInt32 type, byte padbits = 0, string postfix = null)
      {
         UInt32 mask = (UInt32)((1 << (inclusive_end + 1 - start)) - 1);
         return new ArchOpField(label, mask, (byte)(insn_size - inclusive_end - 1), type, padbits, postfix);
      }

      public ArchOpField(string label, UInt32 bits, byte start, UInt32 type, byte padbits = 0, string postfix = null)
      {
         _label = label;
         _postfix = postfix;
         _bits = bits;
         _start = start;
         _padbits = padbits;
         _type = type;
      }

      public UInt64 DECODE(UInt64 raw_insn)
      {
         UInt64 val = ((raw_insn >> _start) & _bits) << _padbits;
         if ((_type & ArchEmulator.FT_Signed) > 0)
            val = EXTS(val);

         return val;
      }
      public UInt64 EXTS(UInt64 val)
      {
         //Sign extending algorithm
         UInt64 sgn_mask = _bits;
         sgn_mask = (sgn_mask >> 32) | (sgn_mask >> 16) | (sgn_mask >> 8) | (sgn_mask >> 4) | (sgn_mask >> 2) | (sgn_mask >> 1);
         if ((val & (sgn_mask ^ (sgn_mask >> 1))) > 0)
            val |= (~sgn_mask);
         return val;
      }

      public bool TryWrite(ArchEmulator emul, UInt64 loc, UInt64 val, UInt64 mask = UInt64.MaxValue, byte shft = 0)
      {
         bool success = false;
         if ((_type & ArchEmulator.FT_Reg) > 0)
         {
            if ((_type & ArchEmulator.FT_GEN) > 0)
            {
               emul.GenPR[loc] &= ~mask;
               emul.GenPR[loc] |= val;
               success = true;
            }
            else if ((_type & ArchEmulator.FT_COND) > 0)
            {
               emul.CondR &= ~mask;
               emul.CondR |= val;
               success = true;
            }
            else if ((_type & ArchEmulator.FT_SP) > 0)
            {
               emul.SpecPR[loc] &= ~mask;
               emul.SpecPR[loc] |= val;
               success = true;
            }
         }

         return success;
      }

      public bool TryLoad(ArchEmulator emul, UInt64 loc, UInt64 mask, byte shft, ref UInt64 dword)
      {

         bool success = false;
         if ((_type & ArchEmulator.FT_Reg) > 0)
         {
            if ((_type & ArchEmulator.FT_GEN) > 0)
            {
               dword = (emul.GenPR[loc] & mask);
               success = true;
            }
            else if ((_type & ArchEmulator.FT_COND) > 0)
            {
               dword = (emul.CondR & mask);
               success = true;
            }
            else if ((_type & ArchEmulator.FT_SP) > 0)
            {
               dword = (emul.SpecPR[loc] & mask);
               success = true;
            }
         }

         if (success)
         {
            if (shft < 0)
               dword >>= -shft;
            else
               dword <<= shft;
         }

         return success;
      }

      public void DISPLAY(StringBuilder sb, UInt64 val)
      {
         if ((_type & ArchEmulator.FT_Hidden) > 0)
            return;

         else if ((_type & ArchEmulator.FT_Reg) > 0)
         {
            if ((_type & ArchEmulator.FT_GEN) > 0)
            {
               sb.Append("gpr");
               sb.Append(val);
            }
            else if ((_type & ArchEmulator.FT_COND) > 0)
            {
               sb.Append("cr");
               sb.Append(val);
            }
            else if ((_type & ArchEmulator.FT_SP) > 0)
            {
               sb.Append("spr");
               sb.Append(val);
            }
         }
         else if ((_type & ArchEmulator.FT_Immd) > 0)
         {
            if (((_type & ArchEmulator.FT_Signed) > 0) &&
                ((val & 0x8000000000000000) > 0))
            {
               sb.AppendFormat("-0x{0:X}", 0 - val);
            }
            else sb.AppendFormat("0x{0:X}", val);
         }
         else sb.Append(val);
      }
   }

   public class ArchOpGroup
   {
      public UInt32 _opcode;
      public UInt32 _opcode_mask;
      public UInt32 _ext_opcode_mask;
      public ArchOp[] _insns;

      public ArchOpGroup(UInt32 opcode, UInt32 opcode_mask, UInt32 ext_opcode_mask, params ArchOp[] insns)
      {
         _opcode = opcode;
         _opcode_mask = opcode_mask;
         _ext_opcode_mask = ext_opcode_mask;
         _insns = insns;
      }
   }

   public class ArchForm
   {
      public UInt64 _ext_opcode_mask;
      public UInt16[] _fields;
   }

   public class ArchOp
   {
      public static ArchOp TODO = new ArchOp("TODO","not implemented",-1, 0, 0, 0, UInt32.MaxValue, null);
      public string _name;
      public string _fullname;
      public Action<ArchEmulator> _emul;
      public UInt64 _ext_opcode_mask;
      public UInt64 _opcode;
      public UInt32 _isaflags;
      public UInt32 _optype;
      public UInt16[] _fields;
      public sbyte _opsize;


      public ArchOp(string name, string fullname, sbyte opsize, UInt64 ext_opcode_mask, UInt64 opcode, UInt32 optype, UInt32 isaFlags, Action<ArchEmulator> emul, params UInt16[] fields)
      {
         _name = name;
         _fullname = fullname;
         _opcode = opcode;
         _fields = fields;
         _opsize = opsize;
         _emul = emul;
         _optype = optype;
         _isaflags = isaFlags;
         _ext_opcode_mask = ext_opcode_mask;
      }

      public void DECODE(ArchEmulator emul)
      {
         int max_fields = _fields.Length - 1;
         Debug.Assert(max_fields < emul.OPFs.Length - 1);
         emul.OPPostfix = "";
         for(int ii = 0; ii <= max_fields; ii++)
         {
            ArchOpField field = emul.Fields[_fields[ii]];
            emul.OPFs[ii] = field.DECODE(emul.OPRaw);

            if (field._postfix != null && emul.OPFs[ii] == 1)
               emul.OPPostfix += field._postfix;
         }
      }

      public void DISPLAY(ArchEmulator emul, VariableTable vartab, StringBuilder sb)
      {
         sb.Append(_name);
         sb.Append(emul.OPPostfix);
         sb.Append(' ');
         int max_fields = _fields.Length - 1;

         Debug.Assert(max_fields < emul.OPFs.Length - 1);
         for(int ii = 0; ii <= max_fields; ii++)
         {
            ArchOpField field = emul.Fields[_fields[ii]];
            if ((ii != 0) && ((field._type & ArchEmulator.FT_Hidden) == 0))
               sb.Append(',');

            field.DISPLAY(sb, emul.OPFs[ii]);
         }
      }
   }
}
