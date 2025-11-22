# Semantic Architecture
## Libs to Use

- CoreSpec will pull in an .isa/.isaext set, define a register set
- From a CoreSpec we can spawn a CoreState which will hold the "register file" with register access methods through a data handle "src/soc/system/bus/data.rs" with a basic memory backing.  
- Bitfield in soc/prog/types/bitfield.rs handles padding, sign and zero extension. read_signed will be used extensively when parsing instruction bytes and getting arguments in decimal usable form 

## Operations highlighted

- Macro calling $macro::<tag>(args)
- New "host" space for calling pc functions $host::<host func tag>(args)
- getting at the value in an arg or a param #<argtag> #<paramtag>
- Reading or writing a field $reg::GPR(<index>) (could also add ::msb to get at the msb subfield) (uses the register file to update the state)
- Previous instruction definitions being "callable" $<spacetag>::<insntag>(args)
- (a,b) tuple returns.  

## Example
:space reg addr=32 word=64 type=register align=16 endian=big
:space insn addr=32 word=32 type=logic align=16 endian=big

//### Registers (in register space) #####
:reg GPR[0..31] offset=0x0 size=64 reset=0 disp="r%d"
subfields={
    msb @(0..31)
    lsb @(32..63)
}

//Special Purpose Registers

//First define the entire SPR space
:reg SPR[0..1023] offset=0x1000 size=64
subfields={
    msb @(0..31)
    lsb @(32..63)
}

//Use the redirect function to call out specific SPRs and to route them to the same backing memory
:reg XER redirect=SPR1
subfields={
    SO @(32)    descr="Summary Overflow"
    OV @(33)    descr="Overflow"
    CA @(34)    descr="Carry"
    SL @(57..63) descr="String Length"
}

:reg CR[0..7] offset=0x900 size=4
subfields={
    LT @(0) descr="Less Than"
    NEG @(0) descr="Negative"
    GT @(1) descr="Greater Than"
    POS @(1) descr="Positive"
    EQ @(2) descr="Equal"
    ZERO @(2) descr="Zero"
    SO @(3) descr="Overflow"
}

// X-Form: Register-to-register operations with extended opcode
:insn X_Form subfields={
    OPCD @(0..5) op=func descr="Primary opcode"
    RT @(6..10) op=target|$reg::GPR descr="Target register"  
    RA @(11..15) op=source|$reg::GPR descr="Source register A"
    RB @(16..20) op=source|$reg::GPR descr="Source register B"
    XO @(21..30) op=func descr="Extended opcode"
    Rc @(31) op=func descr="Record condition"
} disp="#RT, #RA, #RB"

:macro upd_cr0(res) {
    $reg::CR0::NEG = #res < 0
    $reg::CR0::POS = #res > 0
    $reg::CR0::ZERO = #res == 0
    $reg::CR0::SO = $reg::XER::SO
}

//EREF 2.0 Rev.0 pg.5-12
:insn::X_Form add mask={OPCD=31, XO=266, Rc=0} descr="Add (X-Form)" op="+" semantics={ 
    a = $reg::GPR(#RA)  //treat as a read
    b = $reg::GPR(#RB)
    (res,carry) = $host::add_with_carry(a,b,#SIZE_MODE)
    $reg::GPR(#RT) = res
    (res,carry) //returning res, carry in tuple
}
:insn::X_Form add. mask={OPCD=31, XO=266, Rc=1} descr="Add and record (X-Form)" op="+" semantics={
    $insn::add(#RT,#RA,#RB)    // We want all the mutation effects of "add"
    $macro::upd_cr0(res)        // plus we want to update our CR0 registers
}