using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace EmbedEmul.Types
{
    public class BitConstruct
    {
        public UInt16 ClassSize { get; init; }
        List<BitSlice> Slices;

        UInt16 _maskSize = 0;
        public UInt16 MaskSize { get { return _maskSize; } }

        public BitConstruct(UInt16 classSize)
        {
            ClassSize = classSize;
            Slices = new List<BitSlice>();
        }

        public void AddSlice(BitSlice slice)
        {
            if (slice.Size > ClassSize)
                throw new ArgumentException($"Slice size {slice.Size} exceeds class size {ClassSize}.");

            if (slice.Shift >= ClassSize)
                throw new ArgumentException($"Slice shift {slice.Shift} exceeds class size {ClassSize}.");

            Slices.Add(slice);
        }

        public UInt64 ReadFrom(UInt64 source)
        {
            UInt64 result = 0;
            foreach (var slice in Slices)
            {
                result = slice.AppendTo(source, result);
            }
            return result;
        }

        public void WriteOver(UInt64 target, UInt64 source)
        {
            UInt64 sliceVal;
            for (int ii = Slices.Count - 1; ii >= 0; ii--)
            {
                sliceVal = Slices[ii].UndoAppend(ref source);
                target = Slices[ii].WriteOver(target, sliceVal);
            }
        }
    }
    public struct BitSlice
    {
        public static BitSlice Zero = new BitSlice()
        {
            Mask = 0,
            Size = 0,
            Shift = 0,
        };

        public UInt64 Mask { get; init; }
        public UInt16 Size { get; init; }
        public UInt16 Shift { get; init; }

        public bool IsPad { get { return Mask == 0; } }

        public static BitSlice CreateFlag(UInt16 classSize, UInt16 bitNumber)
        {
            return CreateSlice(classSize, bitNumber, bitNumber);
        }

        public static BitSlice CreateSlice(UInt16 classSize, UInt16 start, UInt16 end)
        {
            Debug.Assert(end >= start);
            Debug.Assert(classSize > end);

            ushort size = (ushort)(start - end + 1);
            ushort shift = (ushort)(classSize - end - 1);
            UInt64 mask = (UInt64)(((1 << size) - 1) << shift);

            return new BitSlice()
            {
                Size = size,
                Shift = shift,
                Mask = mask
            };
        }

        public static BitSlice CreatePad(UInt16 bitCount, UInt16 shift)
        {
            return new BitSlice()
            {
                Mask = 0,
                Size = bitCount,
                Shift = shift
            };
        }

        public UInt64 UndoAppend(ref UInt64 source)
        {
            UInt64 sliceValue = source & (Mask >> Shift);
            source >>= Size;
            return sliceValue;
        }

        public UInt64 AppendTo(UInt64 source, UInt64 startValue)
        {
            return (startValue << Size) | ReadFrom(source);
        }

        public UInt64 ReadFrom(UInt64 source)
        {
            return (source & Mask) >> Shift;
        }

        public UInt64 WriteOver(UInt64 target, UInt64 source)
        {
            return (target & ~Mask) | ((source << Shift) & Mask);
        }
    }
}