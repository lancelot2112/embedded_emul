using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using EmbedEmul.Memory;

namespace EmbedEmul.Programs.Binary
{
    public static class BinFileBlocks
    {
        public static HashSet<string> VALID_EXTENSIONS = new HashSet<string>()
        {
            ".bin",
            ".bin.signed",
            ".signed"
        };

        public static BinaryImage FromFiles(string[] filePaths,UInt64[] startAddresses,ByteOrder[] orders,StatusUpdateDelegate statusHandlers = null)
        {
            BinaryImage file = new BinaryImage();
            int ii = 0;
            foreach (string path in filePaths)
            {
                byte[] block = BinFile.FromFile(path,statusHandlers);
                MemoryUnit memBlock = new MemoryUnit(block,orders[ii],startAddresses[ii]);
                file.AddBlock(memBlock);
                ii++;
            }
            return file;
        }

        public static void ToFiles(BinaryImage file,string[] filePaths = null)
        {
            if(filePaths == null)
            {
                filePaths = new string[file.Blocks.Count()];
                for (int ii = 0; ii < file.Blocks.Count(); ii++)
                    filePaths[ii] = file._fileInfo.Directory + file._fileInfo.Name + "_blk" + ii.ToString() + ".bin";
            } else if (filePaths.Length != file.Blocks.Count()) {
                file.OnStatusUpdate(file,"BinFileBlocks.ToFiles","Number of file paths does not match number of blocks.",StatusUpdateType.Error);
                return;
            }

            int pathIdx = 0;
            foreach(MemoryUnit block in file.Blocks)
            {
                BinFile.ToFile(filePaths[pathIdx],block.Data);
                pathIdx++;
            }
        }
    }
}
