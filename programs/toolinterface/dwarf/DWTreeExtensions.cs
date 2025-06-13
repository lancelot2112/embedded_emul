using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Programs.TIS.Dwarf
{
    public static class DWTreeExtensions
    {
        public static IEnumerable<DWDie> Descendants(this IEnumerable<DWDie> entries)
        {
            foreach (DWDie entry in entries)
                foreach (DWDie descendant in Descendants(entry))
                    yield return descendant;
        }

        public static IEnumerable<DWDie> Descendants(this DWDie entry)
        {
            DWCompilationUnitHeader cu = entry._cu;
            UInt32 pos = entry._offset + entry._length;
            //if (cu._context._dwarfVersion > 1)
            //    pos -= cu._start;
            UInt32 siblingpos = entry._sibling; //_siblingPos(entry);
            DWDie descendant;
            
            while (pos < siblingpos)
            {
                if (pos >= cu._debugInfo._data.LongLength) break;
                descendant = cu.GetDIE(pos);
                yield return descendant;
                pos += descendant._length;
            }
        }

        public static DWDie Reference(this DWDie entry, UInt32 offset)
        {
            DWCompilationUnitHeader cu = entry._cu;
            return cu.GetDIE(offset);
        }

        public static IEnumerable<DWDie> DescendantsAndSelf(this IEnumerable<DWDie> entries)
        {
            foreach (DWDie entry in entries)
            {
                yield return entry;
                foreach (DWDie descendant in Descendants(entry))
                    yield return descendant;
            }
        }

        public static IEnumerable<DWDie> Children(this DWDie entry)
        {
            DWCompilationUnitHeader cu = entry._cu;
            UInt32 pos = entry._offset + entry._length;
            //if (cu._context._dwarfVersion > 1)
            //    pos -= cu._start;
            UInt32 siblingpos = entry._sibling; //_siblingPos(entry);
            DWDie child;
            if (pos < siblingpos || entry._children == DWChildren.DW_CHILDREN_yes)
            {
                child = cu.GetDIE(pos);
                while (child.Tag != DWTag.DW_TAG_padding)
                {
                    yield return child;

                    pos = child._sibling; //_siblingPos(child);
                    if (pos >= cu._debugInfo._data.LongLength) break;

                    child = cu.GetDIE(pos);
                }
            }

            List<UInt32> indirectChildren;
            if(cu._indirectMemberCache.TryGetValue(entry._offset, out indirectChildren))
            {
                foreach (UInt32 offset in indirectChildren)
                    yield return cu.GetDIE(offset);
            }
        }

        //private static UInt32 _siblingPos(DWDie entry)
        //{
        //    UInt32 pos;
        //    DWAttribute attr;
        //    if (entry.TryGetAttribute(DWAttrType.DW_AT_sibling, out attr))
        //    {
        //        pos = (uint)entry.GetUData(DWAttrType.DW_AT_sibling);
        //    }
        //    else
        //    {
        //        pos = entry._offset + entry._length;
        //        //if (entry._cu._context._dwarfVersion > 1)
        //        //    pos -= entry._cu._start;
        //    }

        //    return pos;

        //}

        public static IEnumerable<DWDie> SiblingsAfter(this IEnumerable<DWDie> entries)
        {
            foreach (DWDie entry in entries)
                foreach (DWDie sibling in SiblingsAfter(entry))
                    yield return sibling;
        }

        public static IEnumerable<DWDie> SiblingsAfter(this DWDie entry)
        {
            UInt32 nextsiblingpos;
            DWDie sibling = entry;
            DWCompilationUnitHeader cu = entry._cu;
            while (sibling.Tag != DWTag.DW_TAG_padding)
            {
                nextsiblingpos = entry._sibling; //_siblingPos(entry);

                if (nextsiblingpos >= cu._debugInfo._data.LongLength) break;
                sibling = cu.GetDIE(nextsiblingpos);
                if (sibling.Tag != DWTag.DW_TAG_padding) yield return sibling;
            }
        }

        public static IEnumerable<DWDie> SiblingsAfterAndSelf(this IEnumerable<DWDie> entries)
        {
            foreach (DWDie entry in entries)
            {
                yield return entry;
                foreach (DWDie sibling in SiblingsAfter(entry))
                    yield return sibling;
            }
        }
    }
}
