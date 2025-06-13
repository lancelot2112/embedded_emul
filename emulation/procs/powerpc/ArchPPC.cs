using System;
using System.Collections.Generic;
using System.IO;

using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;
using GenericUtilitiesLib;
using EmbedEmul.Binary;
using EmbedEmul.Variables;

namespace EmbedEmul.Hardware
{
   public class ArchPPC : ArchEmulator
   {
      public ArchPPC()
      {
         OC_32bit = OC5;
      }
      /* Basic Instruction Form
       * 0     6
       * [OPCD][ARGS]
       */

      /***  Fields (F) index list for indexing into the field definition arrays ***/
      const int F_NULL = 0;
      const int F_AA =     F_NULL + 1;   //Absolute Address
      const int F_ARX = F_AA + 1; //(12:15)
      const int F_ARY = F_ARX + 1;
      const int F_BA =     F_ARY + 1;     //Bit in CR to be used as source (A)
      const int F_BB =     F_BA + 1;     //Bit in CR to be used as source (B)
      const int F_BD =     F_BB + 1;     //14-bit signed 2's cmplnt branch displacement concat-right b00 and sign-ext to 64-bits
      const int F_BD8 = F_BD + 1;
      const int F_BD15 = F_BD8 + 1;
      const int F_BD24 = F_BD15 + 1;
      const int F_BF =     F_BD24 + 1;     //One of the CR or FPSCR fields to be used as target
      const int F_BF32 = F_BF + 1; //(9:10)
      const int F_BFA =    F_BF32 + 1;    //One of the CR or FPSCR fields to be used as source
      const int F_BH =     F_BFA + 1;    //Hint for branch conditional register
      const int F_BI =     F_BH + 1;     //Bit in CR to be tested by branch conditional
      const int F_BI16 = F_BI + 1;  //(6:7)
      const int F_BI32 = F_BI16 + 1; //(12:15)
      const int F_BO =     F_BI32 + 1;     //Specify options for branch conditional
      const int F_BO16 = F_BO + 1; //(5)
      const int F_BO32 = F_BO16 + 1;    //(10:11)
      const int F_BT =     F_BO32 + 1;     //Bit in CR or FPSCR to be used as target
      const int F_D =      F_BT + 1;      //Immediate 16-bit signed 2's cmpl int sign-ext to 64-bits
      const int F_D8 = F_D + 1; //(24:31)
      const int F_DS =     F_D8 + 1;      //Immediate 14-bit signed 2's cmpl int concat-right b00 and sign-ext to 64-bits
      const int F_F = F_DS + 1; //(21)
      const int F_FLM =    F_F + 1;    //Field mask to identify FPSCR fields
      const int F_FRA =    F_FLM + 1;   //Floating Point Reg Source (A)
      const int F_FRB =    F_FRA + 1;   //Floating Point Reg Source (B)
      const int F_FRC =    F_FRB + 1;   //Floating Point Reg Source (C)
      const int F_FRS =    F_FRC + 1;   //Floating Point Reg Source
      const int F_FRT =    F_FRS + 1;   //Floating Point Reg target
      const int F_FXM =    F_FRT + 1;   //Field mask to identify CR fields
      const int F_L =      F_FXM + 1;     //Length for fixed point compare 32 or 64 bits (10)
      const int F_L_OE =   F_L + 1;       //Field used by move to MSR and TLB (15)
      const int F_L_SYNC = F_L_OE + 1;    //FIeld used by sync instruction (9:10)
      const int F_LEV =    F_L_SYNC + 1;     //Load for sys call fcn
      const int F_LI =     F_LEV + 1;    //Load Immediate
      const int F_LI20_1 = F_LI + 1;
      const int F_LI20_2 = F_LI20_1 + 1;
      const int F_LI20_3 = F_LI20_2 + 1;
      const int F_LK =     F_LI20_3 + 1;     //LINK bit
      const int F_LK7 = F_LK + 1; //(7)
      const int F_LK16 = F_LK7 + 1; //(16)
      const int F_OIM5 = F_LK16 + 1; //(7:11)
      const int F_MB =     F_OIM5 + 1; //First 1-bit of 64-bit mask (21:25)
      const int F_ME =     F_MB + 1; //Last 1-bit of 64-bit mask  (26:30)
      const int F_MB_EXT = F_ME + 1; //First 1-bit of 64-bit mask (21:26)
      const int F_ME_EXT = F_MB_EXT + 1;     //Last 1-bit of 64-bit mask (21:26)
      const int F_NB =     F_ME_EXT + 1;     //Number of bytes
      const int F_OE =     F_NB + 1;   //
      const int F_RA =     F_OE + 1;     //General Purpose Reg (A)
      const int F_RB =     F_RA + 1;     //General Purpose Reg (B)
      const int F_Rc = F_RB + 1;     //Record bit to alter CondReg
      const int F_Rc6 = F_Rc + 1;
      const int F_Rc7 = F_Rc6 + 1;
      const int F_Rc20 = F_Rc7 + 1;
      const int F_RS =     F_Rc20 + 1;     //General Purpose Reg Source
      const int F_RT =     F_RS + 1;     //General Purpose Reg Target
      const int F_RX = F_RT + 1; //(12:15)
      const int F_RY = F_RX + 1; //(8:11)
      const int F_RZ = F_RY + 1; //(8:11)
      const int F_SCL = F_RZ + 1; //(22:23)
      const int F_SD4 = F_SCL + 1; //(4:7)
      const int F_SH =     F_SD4 + 1;     //Shift amount (16:20)
      const int F_SH_S =   F_SH + 1;   //Shift amount split (16:20 - 30)
      const int F_SI =     F_SH_S + 1;     //Immediate 16-bit signed integer (16:31)
      const int F_SI6 = F_SI + 1; //(6:10)
      const int F_SI11 = F_SI6 + 1;  //(11:15)
      const int F_SI21 = F_SI11 + 1; //(21:31)
      const int F_SPR =    F_SI + 1;    //Special Purpose Register
      const int F_SR =     F_SPR + 1;    //Segment Register
      const int F_TBR =    F_SR + 1;    //Time base move
      const int F_TH =     F_TBR + 1;    //
      const int F_TO =     F_TH + 1;     //Conditions on which to trap
      const int F_U =      F_TO + 1;      //Immediate field placed in field in FPSCR
      const int F_UI =     F_U + 1;      //Immediate unsigned 16-bit integer (16:31)
      const int F_UI5 = F_UI + 1; //(7:11) 5-bit unsigned number
      const int F_UI6 = F_UI5 + 1; //(6:10)
      const int F_UI7 = F_UI6 + 1; //(5:11) 7-bit unsigned number
      const int F_UI8 = F_UI7 + 1; //(24:31) 8-bit unsigned number
      const int F_UI11 = F_UI8 + 1; //(11:15)
      const int F_UI21 = F_UI11 + 1; //(21:31)
      //const int F_XO =     F_UI + 1;     //Extended opcode field

      public override ArchOpField[] Fields { get { return field_list; } }
      static ArchOpField[] field_list = new ArchOpField[]
      {
          new ArchOpField("NULL", 0,0,FT_NOP), //NULL
          ArchOpField.Create("AA", 30,30,32,FT_Options | FT_Hidden,postfix:"a"), //AA (30) Absolute Address
          ArchOpField.Create("ARX", 12, 15, 16, 0), //(12:15) 16-bit
          ArchOpField.Create("ARY", 8, 11, 16 ,0), //(8:11)  16-bit
          ArchOpField.Create("BA", 11,15,32,FT_COND | FT_Reg), //BA (11:15)
          ArchOpField.Create("BB", 16,20,32,FT_COND | FT_Reg), //BB (16:20)
          ArchOpField.Create("BD", 16,29,32,FT_Immd | FT_Signed, padbits:2), //BD (16:29) value || 00b
          ArchOpField.Create("BD8", 8,15,16,FT_Immd | FT_Signed, padbits:2), //BD8 (8:15) value || 00b  16-bit
          ArchOpField.Create("BD15", 16,30,32,FT_Immd | FT_Signed, padbits:2), //BD15 (16:30) value || 00b
          ArchOpField.Create("BD24", 7,30,32,FT_Immd | FT_Signed, padbits:2), //BD24 (7:30) value || 00b
          ArchOpField.Create("BF", 6,8,32,FT_COND | FT_Reg), //BF (6:8) floating condition reg
          ArchOpField.Create("BF32", 9,10,32 ,0), //(9:10)
          ArchOpField.Create("BFA", 11,13,32,FT_COND | FT_Reg), //BFA (11:13)
          ArchOpField.Create("BH", 19,20,32,FT_Hint), //BH (19:20)
          ArchOpField.Create("BI", 11,15,32,FT_COND | FT_Reg), //BI (11:15)
          ArchOpField.Create("BI16", 6,7,16 ,0), //(6:7)  16-bit
          ArchOpField.Create("BI32", 12,15,32 ,0), //(12:15)
          ArchOpField.Create("BO", 6,10,32,FT_Options), //BO (6:10)
          ArchOpField.Create("BO16", 5,5,16 ,0), //(5) 16-bit
          ArchOpField.Create("BO32", 10,11,32 ,0), //(10:11)
          ArchOpField.Create("BT", 6,10,32,FT_Mask), //BT (6:10)
          ArchOpField.Create("D", 16,31,32,FT_Immd), //D (16:31)
          ArchOpField.Create("D8", 24,31,32 ,0), //(24:31)
          ArchOpField.Create("DS", 16,29,32,FT_Immd), //DS (16:29)
          ArchOpField.Create("F", 21,21,32 ,0), //(21)
          ArchOpField.Create("FLM", 7,14,32,FT_Mask | FT_COND | FT_Reg), //FLM (7:14)
          ArchOpField.Create("FRA", 11,15,32,FT_FLOAT | FT_Reg), // FRA (11:15)
          ArchOpField.Create("FRB", 16,20,32,FT_FLOAT | FT_Reg), //FRB (16:20)
          ArchOpField.Create("FRC", 21,25,32,FT_FLOAT | FT_Reg), //FRC (21:25)
          ArchOpField.Create("FRS", 6,10,32,FT_FLOAT | FT_Reg), //FRS (6:10)
          ArchOpField.Create("FRT", 6,10,32,FT_FLOAT | FT_Reg), //FRT (6:10)
          ArchOpField.Create("FXM", 12,19,32,FT_Mask | FT_COND | FT_Reg), //FXM (12:19)
          ArchOpField.Create("L", 10,10,32,FT_Options), //L (10)
          ArchOpField.Create("L_OE", 15,15,32,FT_Options), //L_OE (15)
          ArchOpField.Create("L_SYNC", 9,10,32,FT_Options),  //L_SYNC (9:10)
          ArchOpField.Create("LEV", 20,26,32,FT_Options), //LEV (20:26)
          ArchOpField.Create("LI", 6,29,32,FT_Immd | FT_Signed,padbits:2), //LI (6:29) padded on right with 00b and sign extended
          ArchOpField.Create("LI20_1", 17,20,32, 0), //(17:20)
          ArchOpField.Create("LI20_2", 11,15,32, 0), //(11:15)
          ArchOpField.Create("LI20_3", 21,31,32, 0), //(21:31)
          ArchOpField.Create("LK", 31,31,32,FT_Options | FT_Hidden,postfix:"l"), //LK (31)
          ArchOpField.Create("LK7", 7,7,32,0),  //(7)
          ArchOpField.Create("LK15", 15,15,16 ,0), //(15) 16-bit
          ArchOpField.Create("OIM5", 7,11,16 ,0), //(7:11) 16-bit
          ArchOpField.Create("MB", 21,25,32,FT_Mask), //MB (21:25)
          ArchOpField.Create("ME", 26,30,32,FT_Mask), //ME (26:30)
          ArchOpField.Create("MB_EXT", 21,26,32,FT_Mask), //MB_EXT (21:26)
          ArchOpField.Create("ME_EXT", 21,26,32,FT_Mask), //ME_EXT (21:26)
          ArchOpField.Create("NB", 16,20,32,FT_Immd), //NB (16:20)
          ArchOpField.Create("OE", 21,21,32,FT_Options | FT_Hidden,postfix:"o"), //OE (21)
          ArchOpField.Create("RA", 11,15,32,FT_GEN | FT_Reg), //RA (11:15)
          ArchOpField.Create("RB", 16,20,32,FT_GEN | FT_Reg), //RB (16:20)
          ArchOpField.Create("Rc", 31,31,32,FT_Options | FT_Hidden,postfix:"."), //Rc (31)
          ArchOpField.Create("Rc6", 6,6,16,FT_Options | FT_Hidden,postfix:"."), //Rc (6) 16-bit
          ArchOpField.Create("Rc7", 7,7,16,FT_Options | FT_Hidden,postfix:"."), //Rc (7) 16-bit
          ArchOpField.Create("Rc20", 20,20,32,FT_Options | FT_Hidden,postfix:"."), //Rc (20)
          ArchOpField.Create("RS", 6,10,32,FT_GEN | FT_Reg), //RS (6:10)
          ArchOpField.Create("RT", 6,10,32,FT_GEN | FT_Reg), //RT (6:10)
          ArchOpField.Create("RX", 12,15,16 ,0), //(12:15) 16-bit
          ArchOpField.Create("RY", 8,11,16 ,0), //(8:11) 16-bit
          ArchOpField.Create("RZ", 8,11,16,0), //(8:11) 16-bit
          ArchOpField.Create("SCL", 22,23,32 ,0), //(22:23)
          ArchOpField.Create("SD4", 4,7,16 ,0), //(4:7) 16-bit
          ArchOpField.Create("SH", 16,20,32,FT_Immd), //SH (16:20)
          ArchOpField.Create("SH_S", 30,30,32,FT_Immd), //SH_S (30)
          ArchOpField.Create("SI", 16,31,32,FT_Immd), //SI (16:31)
          ArchOpField.Create("SI6", 6,10,32,0),  //(6:10)
          ArchOpField.Create("SI11", 11,15,32,0),  //(11:15)
          ArchOpField.Create("SI21", 21,31,32,0),   //(21:31)
          ArchOpField.Create("SPR", 11,20,32,FT_SP | FT_Reg), //SPR (11:20)
          ArchOpField.Create("SR", 12,15,32,FT_SEG | FT_Reg), //SR (12:15)
          ArchOpField.Create("TBR", 11,20,32,FT_Options), //TBR (11:20)
          ArchOpField.Create("TH", 9,10,32,FT_Options), //TH (9:10)
          ArchOpField.Create("TO", 6,10,32,FT_Trap), //TO (6:10)
          ArchOpField.Create("U", 16,19,32,FT_Immd | FT_SP | FT_Reg), //U (16:19)
          ArchOpField.Create("UI", 16,31,32,FT_Immd), //UI (16:31)
          ArchOpField.Create("UI5", 7,11,16 ,0),  //(7:11) 16-bit
          ArchOpField.Create("UI6", 6,10,16,0),  //(6:10)
          ArchOpField.Create("UI7", 5,11,16 ,0),  //(5:11) 16-bit
          ArchOpField.Create("UI8", 24,31,32 ,0),  //(24:31)
          ArchOpField.Create("UI11", 11,15,32 ,0),  //(11:15)
          ArchOpField.Create("UI21", 21,21,32 ,0),  //(21:21)
      };


      const UInt32 OCNull = 0;
      const UInt32 OC3 = 0xf000; //OPCODE mask (0:3) VLE
      const UInt32 OC4 = 0xf800; //OPCODE mask (0:4) VLE
      const UInt32 OC5 = 0xfc000000; //OPCODE mask (0:5)
      const UInt32 OC9 = 0xffc0; //OPCODE mask (0:9) VLE
      const UInt32 OC14 = 0xfffe; //OPCODE mask (0:14) VLE
      const UInt32 OC15 = 0xffff; //OPCODE mask (0:15) VLE

      const int OC3sh = 12;
      const int OC4sh = 11;
      const int OC5sh = 26;
      const int OC9sh = 6;
      const int OC14sh = 1;
      const int OC15sh = 0;

      const UInt32 OCAA = 0x2; //AA abs address mask (30)
      const UInt32 OCBO = 0x03e00000; //BO branch options for extended mnemonics (6:10)
      const int OCBOsh = 21;
      const int OCAAsh = 1;
      const UInt32 OCTO = 0x03e00000; //TO trap mask (6:10)
      const int OCTOsh = 21;
      const UInt32 OCLK = 0x1; //LK link mask (31)
      const UInt32 OCX2129 = 0x7fc; //XO mask (21:29)
      const int OCX29sh = 2;
      const UInt32 OCX2130 = 0x7fe; //XO mask (21:30)
      const int OCX30sh = 1;
      const UInt32 OCX2230 = 0x3fe; //XO mask (22:30)
      const UInt32 OCX2630 = 0x3e; //XO mask (26:30)
      const UInt32 OCX2729 = 0x1c; //XO mask (27:29)
      const UInt32 OCX2730 = 0x1e; //XO mask (27:30)
      const UInt32 OCX3031 = 0x3; //XO mask (30:31)
      const UInt32 OCRc = 0x1; //Rc mask (31)
      const UInt32 OCRcsh = 0x0;
      const UInt32 OCOE = 0x400;
      const UInt32 OCOEsh = 10;

      const UInt32 ISA_ALL = UInt32.MaxValue;
      const UInt32 ISA_32 = 1 << 0;
      const UInt32 ISA_64 = 1 << 1;
      const UInt32 ISA_PWR1 = 1 << 2;
      const UInt32 ISA_PWR2 = 1 << 3;
      const UInt32 ISA_PWR3 = 1 << 4;
      const UInt32 ISA_PWR4 = 1 << 5;
      const UInt32 ISA_VLE = 1 << 6;
      const UInt32 ISA_VEC = 1 << 7;

      const UInt64 MSB_64 = 0x8000000000000000;
      const UInt64 MSB_32 = 0x80000000;
      const UInt64 MSB_24 = 0x80000;
      const UInt64 MSB_16 = 0x8000;
      const UInt64 MSB_8 = 0x80;

      public override ArchOpGroup[] Ops { get { return op_group_list; } }
        //pg. 65
      static ArchOpGroup[] op_group_list = new ArchOpGroup[]
      {
         new ArchOpGroup(2U<<OC5sh, OC5,OCNull,
            new ArchOp("tdi","Trap Doubleword Immediate",4,0,0,IT_TRAP,ISA_ALL,null,F_TO,F_RA,F_SI)),
         new ArchOpGroup(3U<<OC5sh, OC5,OCNull,
            new ArchOp("twi","Trap Word Immediate",4,0,0,IT_TRAP,ISA_ALL,null,F_TO,F_RA,F_SI)),
         new ArchOpGroup(7U<<OC5sh,OC5,OCNull,
             new ArchOp("mulli","Multiply Low Immediate",4,0, 0,IT_TFORM,ISA_ALL,null, F_RT,F_RA,F_SI)),
         new ArchOpGroup(8U<<OC5sh, OC5,OCNull,
             new ArchOp("subfic","Subtract From Immediate Carry",4, 0,0,IT_TFORM,ISA_ALL,null, F_RT,F_RA,F_SI)),
         new ArchOpGroup(10U<<OC5sh, OC5,OCNull,
            new ArchOp("cmpli","Compare Logical Immediate",4,0,0,IT_CMP,ISA_ALL,null,F_BF,F_L,F_RA,F_UI)),
         new ArchOpGroup(11U<<OC5sh, OC5,OCNull,
            new ArchOp("cmpi","Compare Immediate",4,0,0,IT_CMP,ISA_ALL,null, F_BF,F_L,F_RA,F_SI)),
         new ArchOpGroup(12U<<OC5sh,OC5,OCNull,
             new ArchOp("addic","Add Immediate Carrying",4, 0,0,IT_TFORM,ISA_ALL,null, F_RT,F_RA,F_SI)),
         new ArchOpGroup(13U<<OC5sh,OC5,OCNull,
             new ArchOp("addic.","Add Immediate Carrying and Record",4, 0,0,IT_TFORM,ISA_ALL,null, F_RT,F_RA,F_SI)),
         new ArchOpGroup(14U<<OC5sh,OC5,OCNull,
            new ArchOp("addi","Add Immediate", 4,0,0,IT_TFORM,ISA_ALL,null, F_RT,F_RA,F_SI)),
         new ArchOpGroup(15U<<OC5sh,OC5,OCNull,
            new ArchOp("addis","Add Immediate Signed",4, 0,0,IT_TFORM,ISA_ALL,null, F_RT,F_RA,F_SI)),
         new ArchOpGroup(16U<<OC5sh, OC5,OCNull,
            new ArchOp("bc","Branch Conditional",4,0,0,IT_BRANCH|IT_CMP,ISA_ALL,e=>
            {
               UInt64 ctr_msk = UInt64.MaxValue >> ((e.Mode == EmulMode.bits64)?(byte)0:(byte)32);
               bool counter_ok = (e.OPFs[0] & 0x4) == 0x4;
               if(!counter_ok && (e.CountR & ctr_msk) > 0)
               {
                  e.CountR--;
                  counter_ok = ((e.CountR & ctr_msk) > 0) ; //something with BO_3
               }
               bool cond_ok = ((e.OPFs[0] & 0x10) == 0x10) || (((e.CondR & (MSB_32 >> (byte)e.OPFs[1])) >> (31 - (byte)e.OPFs[1])) == ((e.OPFs[0] & 0x8) >> 3));
               e.BranchMode = (counter_ok && cond_ok)?BranchMode.Branch:BranchMode.None;
               if(e.OPFs[4] == 1) e.NIA = e.OPFs[2];
               else e.NIA = e.CIA + e.OPFs[2];
               if(e.OPFs[3]==1) e.LinkR = e.CIA + 4;
            },F_BO,F_BI,F_BD,F_LK,F_AA)),
         new ArchOpGroup(17U<<OC5sh,OC5,OCNull,
            new ArchOp("sc","System Call",4, 0,0,IT_SYS,ISA_ALL,null, F_LEV)),
         new ArchOpGroup(18U<<OC5sh,OC5,OCNull,
            new ArchOp("b","Branch",4,0,0,IT_BRANCH,ISA_ALL,e=>
            {
               if (e.OPFs[2]==1) e.NIA = e.OPFs[0];
               else e.NIA = e.OPFs[0] + e.CIA;
               e.BranchMode = BranchMode.Branch;
               if(e.OPFs[1] == 1) e.LinkR = e.CIA + 4;
            },                F_LI,F_LK,F_AA)), //add add l for link and a for AA
         new ArchOpGroup(19U<<OC5sh, OC5,OCX2130,
            new ArchOp("mcrf","Move Condition Register Field",4,    OCX2130,0,IT_TFORM,ISA_ALL,null,                   F_BF,F_BFA),
            new ArchOp("bclr","Branch Conditional to Link Register",4,    OCX2130,(16<<OCX30sh),IT_BRANCH|IT_CMP,ISA_ALL,null,       F_BO,F_BI,F_BH,F_LK), //Add l for link
            new ArchOp("crnor","Condition Register NOR",4,   OCX2130,(33<<OCX30sh),IT_TFORM,ISA_ALL,null,       F_BT,F_BA,F_BB),
            //new ArchOp("rfdi",    (39<<OCX30sh)),  //E.ED
            new ArchOp("rfmci","Return from Machine Check Interrupt",4,   OCX2130,(39<<OCX30sh),IT_SYS,ISA_ALL,null), //Embedded
            new ArchOp("rfi","Return from Interrupt",4,     OCX2130,(50<<OCX30sh),IT_SYS,ISA_ALL,null),  //Embedded
            new ArchOp("rfci","Return from Critical Interrupt",4,   OCX2130, (51<<OCX30sh),IT_SYS,ISA_ALL,null),  //Embedded
            new ArchOp("rfgi","Return from Guest Interrupt",4,    OCX2130,(102<<OCX30sh),IT_SYS,ISA_ALL,null), //E.HV
            new ArchOp("crandc","Condition Register AND with Complement",4,  OCX2130,(129<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            new ArchOp("isync","Instruction Synchronize",4,   OCX2130,(150<<OCX30sh),IT_MEMSYNC,ISA_ALL,null),
            new ArchOp("crxor","Condition Register XOR",4,   OCX2130,(193<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            //new ArchOp("dnh",     (198<<OCX30sh), F_DUI, F_DCTL), //E.ED
            new ArchOp("crnand","Condition Register NAND",4,  OCX2130,(225<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            new ArchOp("crand","Condition Register AND",4,   OCX2130,(257<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            new ArchOp("creqv","Condition Register Equivalent",4,   OCX2130,(289<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            new ArchOp("crorc","Condition Register OR with Complement",4,   OCX2130,(417<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            new ArchOp("cror","Condition Register OR",4,    OCX2130,(449<<OCX30sh),IT_TFORM,ISA_ALL,null,      F_BT,F_BA,F_BB),
            new ArchOp("bcctr","Branch Conditional to Count Register",4,   OCX2130,(528<<OCX30sh),IT_BRANCH|IT_CMP,ISA_ALL,null,      F_BO,F_BI,F_BH,F_LK)
            ),
         new ArchOpGroup(20U<<OC5sh, OC5,OCNull,
            new ArchOp("rlwimi","Rotate Left Word Immediate then Mask Insert",4, 0,0,IT_ROTSHFT,ISA_ALL,null, F_RS,F_RA,F_SH,F_MB,F_ME,F_Rc)), //Add . for Rc
         new ArchOpGroup(21U<<OC5sh, OC5,OCNull,
            new ArchOp("rlwinm","Rotate Left Word Immediate then AND with Mask",4,0,0,IT_ROTSHFT,ISA_ALL,null, F_RS,F_RA,F_SH,F_MB,F_ME,F_Rc)),
         new ArchOpGroup(23U<<OC5sh, OC5,OCNull,
            new ArchOp("rlwnm","Rotate Left Word then AND with Mask",4,0,0,IT_ROTSHFT,ISA_ALL,null,F_RS,F_RA,F_RB,F_MB,F_ME,F_Rc)),
         new ArchOpGroup(24U<<OC5sh, OC5,OCNull,
            new ArchOp("ori","OR Immediate",4,0,0,IT_LOGICAL,ISA_ALL,null,F_RS,F_RA,F_UI)),
         new ArchOpGroup(25U<<OC5sh, OC5,OCNull,
            new ArchOp("oris","OR Immediate Shifted",4, 0,0,IT_LOGICAL,ISA_ALL,null, F_RS, F_RA, F_UI)),
         new ArchOpGroup(26U<<OC5sh, OC5,OCNull,
            new ArchOp("xori","XOR Immediate",4, 0,0,IT_LOGICAL,ISA_ALL,null, F_RS, F_RA, F_UI)),
         new ArchOpGroup(27U<<OC5sh, OC5,OCNull,
            new ArchOp("xoris","XOR Immediate Shifted",4, 0,0,IT_LOGICAL,ISA_ALL,null, F_RS, F_RA, F_UI)),
         new ArchOpGroup(28U<<OC5sh, OC5,OCNull,
            new ArchOp("andi.","AND Immediate",4,0,0,IT_LOGICAL,ISA_ALL,null,F_RS,F_RA,F_UI)),
         new ArchOpGroup(29U<<OC5sh, OC5,OCNull,
            new ArchOp("andis.","AND Immediate Shifted",4, 0,0,IT_LOGICAL,ISA_ALL,null, F_RS,F_RA,F_UI)),
         new ArchOpGroup(30U<<OC5sh, OC5,OCX2729,
            new ArchOp("rldicl","Rotate Left Doubleword Immediate then Clear Left",4,  OCX2729,(0),IT_ROTSHFT, ISA_ALL,null,              F_RS,F_RA,F_SH,F_MB_EXT,F_SH_S,F_Rc),
            new ArchOp("rldicr","Rotate Left Doubleword Immediate then Clear Right",4,  OCX2729,(1<<OCX29sh),IT_ROTSHFT,ISA_ALL,null,     F_RS,F_RA,F_SH,F_ME_EXT,F_SH_S,F_Rc),
            new ArchOp("rldic","Rotate Left Doubleword Immediate then Clear",4,   OCX2729,(2<<OCX29sh),IT_ROTSHFT,ISA_ALL,null,     F_RS,F_RA,F_SH,F_MB,F_ME,F_Rc),
            new ArchOp("rldimi","Rotate Left Doubleword Immediate then Mask Insert",4,  OCX2729,(3<<OCX29sh),IT_ROTSHFT,ISA_ALL,null,     F_RS,F_RA,F_SH,F_MB_EXT,F_SH_S,F_Rc),
            new ArchOp("rldcl","Rotate Left Doubleword then Clear Left",4,   OCX2729,(8<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,     F_RS,F_RA,F_RB,F_MB_EXT,F_Rc),
            new ArchOp("rldcr","Rotate Left Doubleword then Clear Right",4,   OCX2729,(9<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,     F_RS,F_RA,F_RB,F_ME_EXT,F_Rc)
            ),
         new ArchOpGroup(31U<<OC5sh,OC5,OCX2130,
            new ArchOp("cmp","Compare",4,     OCX2130,(0),IT_CMP,ISA_ALL,null,                     F_BF,F_L,F_RA,F_RB),
            new ArchOp("tw","Trap Word",4,      OCX2130,(4<<OCX30sh),IT_TRAP,ISA_ALL,null,            F_TO,F_RA,F_RB),
            new ArchOp("subfc","Subtract From Carrying",4,   OCX2130,(8<<OCX30sh),IT_TFORM,ISA_ALL,null,            F_RT,F_RA,F_RB,F_OE,F_Rc), //Add o for OE and . for Rc
            new ArchOp("mullhdu","Multiply High Doubleword Unsigned",4, OCX2130,(9<<OCX30sh),IT_TFORM,ISA_ALL,null,            F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("addc","Add Carrying",4,    OCX2130,(10<<OCX30sh),IT_TFORM,ISA_ALL,null,           F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("mullhwu","Multiply High Word Unsigned",4, OCX2130,(11<<OCX30sh),IT_TFORM,ISA_ALL,null,           F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("mfcr","Move from Condition Register",4,    OCX2130,(19<<OCX30sh),IT_CTRL,ISA_ALL,null,           F_RT),
            new ArchOp("lwarx","Load Word and Reserve Indexed",4,   OCX2130,(20<<OCX30sh),IT_LOAD,ISA_ALL,null,           F_RT,F_RA,F_RB),
            new ArchOp("ldx","Load Doubleword Indexed",4,     OCX2130,(21<<OCX30sh),IT_LOAD,ISA_ALL,null,           F_RT,F_RA,F_RB),
            //new ArchOp("icbt",4,    (22<<OCX30sh),ISA_ALL,null,           F_CT,F_RA,F_RB),
            new ArchOp("lwzx","Load Word and Zero Indexed",4,    OCX2130,(23<<OCX30sh),IT_LOAD,ISA_ALL,null,           F_RT,F_RA,F_RB),
            new ArchOp("slw","Shift Left Word",4,     OCX2130,(24<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,           F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("cntlzw","Count Leading Zeros Word",4,  OCX2130,(26<<OCX30sh),IT_LOGICAL,ISA_ALL,null,           F_RS,F_RA,F_Rc),
            new ArchOp("sld","Shift Left Doubleword",4,     OCX2130,(27<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,           F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("and","AND",4,     OCX2130,(28<<OCX30sh),IT_LOGICAL,ISA_ALL,null,           F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("ldepx","Load Doubleword by External PID Indexed",4,   OCX2130,(29<<OCX30sh),IT_LOAD,ISA_ALL,null,           F_RT,F_RA,F_RB),
            new ArchOp("cmpl","Compare Logical",4,    OCX2130,(32<<OCX30sh),IT_TFORM,ISA_ALL,null,           F_BF,F_L,F_RA,F_RB),
            new ArchOp("subf","Subtract From",4,    OCX2130,(40<<OCX30sh),IT_TFORM,ISA_ALL,null,           F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("ldux","Load Doubleword with Update Indexed",4,    OCX2130,(53<<OCX30sh),IT_LOAD,ISA_ALL,null,           F_RT,F_RA,F_RB),
            new ArchOp("cntlzd","Count Leading Zeros Doubleword",4,  OCX2130,(58<<OCX30sh),IT_LOGICAL,ISA_ALL,null,           F_RS,F_RA,F_Rc),
            new ArchOp("andc","AND with Complement",4,    OCX2130,(60<<OCX30sh),IT_LOGICAL,ISA_ALL,null,           F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("td","Trap Doubleword",4,      OCX2130,(68<<OCX30sh),IT_TRAP,ISA_ALL,null,           F_TO,F_RA,F_RB),
            new ArchOp("mullhd","Multiply High Doubleword",4,  OCX2130,(73<<OCX30sh),IT_TFORM,ISA_ALL,null,           F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("mullhw","Multiply High Word",4,  OCX2130,(75<<OCX30sh),IT_TFORM,ISA_ALL,null,           F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("lbzx","Load Byte and Zero Indexed",4,    OCX2130,(87<<OCX30sh),IT_LOAD,ISA_ALL,null,           F_RT,F_RA,F_RB),
            new ArchOp("neg","Negate",4,     OCX2130,(104<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_OE,F_Rc),
            new ArchOp("lbzux","Load Byte and Zero with Update Indexed",4,   OCX2130,(119<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("nor","NOR",4,     OCX2130,(124<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("subfe","Subtract From Extended",4,   OCX2130,(136<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("adde","Add Extended",4,    OCX2130,(138<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("mtcrf","Move to Condition Register Fields",4,   OCX2130,(144<<OCX30sh),IT_CTRL,ISA_ALL,null,          F_RS,F_FXM),
            new ArchOp("stdx","Store Doubleword Indexed",4,    OCX2130,(149<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("stwx","Store Word Indexed",4,    OCX2130,(151<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("stdux","Store Doubleword with Update Indexed",4,   OCX2130,(181<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("stwux","Store Word with Update Indexed",4,   OCX2130,(183<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("subfze","Subtract From Zero Extended",4,  OCX2130,(200<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_OE,F_Rc),
            new ArchOp("addze","Add to Zero Extended",4,   OCX2130,(202<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_OE,F_Rc),
            new ArchOp("stbx","Store Byte Indexed",4,    OCX2130,(215<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("subfme","Subtract From Minus One Extended",4,  OCX2130,(232<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_OE,F_Rc),
            new ArchOp("mulld","Multiply Low Doubleword",4,   OCX2130,(233<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("addme","Add to Minus One Extended",4,   OCX2130,(234<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_OE,F_Rc),
            new ArchOp("mullw","Multiply Low Word",4,   OCX2130,(235<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("stbux","Store Byte with Update Indexed",4,   OCX2130,(247<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("add","Add",4,     OCX2130,(266<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("lhzx","Load Halfword and Zero Indexed",4,    OCX2130,(279<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("eqv","Equivalent",4,     OCX2130,(284<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("lhzux","Load Halfword and Zero with Update Indexed",4,   OCX2130,(311<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("xor","XOR",4,     OCX2130,(316<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("mfspr","Move from Special Purpose Register",4,   OCX2130,(339<<OCX30sh),IT_CTRL,ISA_ALL,null,          F_RT,F_SPR),
            new ArchOp("lwax","Load Word Algebraic Indexed",4,    OCX2130,(341<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("lwaux","Load Word Algebraic with Update Indexed",4,   OCX2130,(373<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("addh","Add Halfword",4,    OCX2130,(394<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("sthx","Store Halfword Indexed",4,    OCX2130,(407<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("orc","OR with Complement",4,     OCX2130,(412<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("sradi","Shift Right Algebraic Doubleword Immediate",4,   OCX2129,(413<<OCX29sh),IT_ROTSHFT,ISA_ALL,null,          F_RS,F_RA,F_SH,F_SH_S,F_Rc), //shorter extended opcode???
            new ArchOp("addhss","Add Halfword Signed Saturate",4,  OCX2130,(426<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("sthux","Store Halfword with Update Indexed",4,   OCX2130,(439<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("or","OR",4,      OCX2130,(444<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("divdu","Divide Doubleword Unsigned",4,   OCX2130,(457<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("addb","Add Byte",4,    OCX2130,(458<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("divwu","Divide Word Unsigned",4,   OCX2130,(459<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("mtspr","Move to Special Purpose Register", 4,   OCX2130,(467<<OCX30sh),IT_CTRL,ISA_ALL,null,          F_RS,F_SPR),
            new ArchOp("nand","NAND",4,    OCX2130,(476<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("divd","Divide Doubleword",4,    OCX2130,(489<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("addbss","Add Byte Signed Saturate",4,  OCX2130,(490<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("divw","Divide Word",4,    OCX2130,(491<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_OE,F_Rc),
            new ArchOp("lswx","Load String Word Indexed",4,    OCX2130,(533<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("lwbrx","Load Word Byte-Reversed Indexed",4,   OCX2130,(534<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("srw","Shift Right Word",4,     OCX2130,(536<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("srd","Shift Right Doubleword",4,     OCX2130,(539<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("lswi","Load String Word Immediate",4,    OCX2130,(597<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_NB),
            new ArchOp("stswx","Store String Word Indexed",4,   OCX2130,(661<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("stwbrx","Store Word Byte-Reversed Indexed",4,  OCX2130,(662<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("stswi","Store String Word Immediate",4,   OCX2130,(725<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_NB),
            new ArchOp("lhbrx","Load Half Word Byte-Reverse Indexed",4,   OCX2130,(790<<OCX30sh),IT_LOAD,ISA_ALL,null,          F_RT,F_RA,F_RB),
            new ArchOp("sraw","Shift Right Algebraic Word",4,    OCX2130,(792<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("srad","Shift Right Algebraic Doubleword",4,    OCX2130,(794<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,          F_RS,F_RA,F_RB,F_Rc),
            new ArchOp("srawi","Shift Right Algebraic Word Immediate",4,   OCX2130,(824<<OCX30sh),IT_ROTSHFT,ISA_ALL,null,          F_RS,F_RA,F_SH,F_Rc),
            new ArchOp("sthbrx","Store Halfword Byte-Reversed Indexed",4,  OCX2130,(918<<OCX30sh),IT_STORE,ISA_ALL,null,          F_RS,F_RA,F_RB),
            new ArchOp("extsh","Extend Sign Halfword",4,   OCX2130,(922<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_Rc),
            new ArchOp("extsb","Extend Sign Byte",4,   OCX2130,(954<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_Rc),
            new ArchOp("addbu","Add Byte Unsigned",4,   OCX2130,(970<<OCX30sh),IT_TFORM,ISA_ALL,null,          F_RT,F_RA,F_RB,F_Rc),
            new ArchOp("tlbwe","TLB Write Entry",4,   OCX2130,(978<<OCX30sh),IT_TLB,ISA_ALL,null),
            new ArchOp("extsw","Extend Sign Word",4,   OCX2130,(986<<OCX30sh),IT_LOGICAL,ISA_ALL,null,          F_RS,F_RA,F_Rc),
            new ArchOp("addbus","Add Byte Unsigned Shifted",4,  OCX2130,(1002<<OCX30sh),IT_TFORM,ISA_ALL,null,         F_RT,F_RA,F_RB,F_Rc)
            ),
         new ArchOpGroup(32U<<OC5sh,OC5,OCNull,
            new ArchOp("lwz","Load Word and Zero",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(33U<<OC5sh,OC5,OCNull,
            new ArchOp("lwzu","Load Word and Zero with Update",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(34U<<OC5sh, OC5,OCNull,
            new ArchOp("lbz","Load Byte and Zero",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(35U<<OC5sh,OC5,OCNull,
            new ArchOp("lbzu","Load Byte and Zero with Update",4,0,0,IT_LOAD,ISA_ALL,null,F_RT,F_RA,F_D)),
         new ArchOpGroup(36U<<OC5sh,OC5,OCNull,
            new ArchOp("stw","Store Word",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(37U<<OC5sh,OC5,OCNull,
            new ArchOp("stwu","Store Word with Update",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(38U<<OC5sh,OC5,OCNull,
            new ArchOp("stb","Store Byte",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(39U<<OC5sh,OC5,OCNull,
            new ArchOp("stbu","Store Byte with Update",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(40U<<OC5sh,OC5,OCNull,
            new ArchOp("lhz","Load Halfword and Zero",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(41U<<OC5sh,OC5,OCNull,
            new ArchOp("lhzu","Load Halfword and Zero with Update",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(42U<<OC5sh,OC5,OCNull,
            new ArchOp("lha","Load Halfword Algebraic",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(43U<<OC5sh,OC5,OCNull,
            new ArchOp("lhau","Load Halfword Algebraic with Update",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(44U<<OC5sh,OC5,OCNull,
            new ArchOp("sth","Store Halfword",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(45U<<OC5sh,OC5,OCNull,
            new ArchOp("sthu","Store Halfword with Update",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(46U<<OC5sh,OC5,OCNull,
            new ArchOp("lmw","Load Multiple Word",4, 0,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_D)),
         new ArchOpGroup(47U<<OC5sh,OC5,OCNull,
            new ArchOp("stmw","Store Multiple Word",4, 0,0,IT_STORE,ISA_ALL,null, F_RS,F_RA,F_D)),
         new ArchOpGroup(58U<<OC5sh,OC5,OCX3031,
            new ArchOp("ld","Load Doubleword",4, OCX3031,0,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_DS),
            new ArchOp("ldu","Load Doubleword with Update",4, OCX3031,1,IT_LOAD,ISA_ALL,null, F_RT,F_RA,F_DS),
            new ArchOp("lwa","Load Word Algebraic",4,OCX3031,2,IT_LOAD,ISA_ALL,null,F_RT,F_RA,F_DS)),
         new ArchOpGroup(62U<<OC5sh,OC5,OCX3031,
            new ArchOp("std","Store Doubleword",4,OCX3031,0,IT_STORE,ISA_ALL,null,F_RS,F_RA,F_DS),
            new ArchOp("stdu","Store Doubleword with Update",4,OCX3031,1,IT_STORE,ISA_ALL,null,F_RS,F_RA,F_DS))
      };
   }
}
