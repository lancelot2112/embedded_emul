using System.IO;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul.Programs.Binary
{

    /// <summary>
    /// Helper functions for reading and writing a BIN file which describes a binary image.
    /// </summary>
    public static class BinFile
    {
        public static HashSet<string> VALID_EXTENSIONS = new HashSet<string>()
        {
            ".bin",
            ".bin.signed",
            ".signed"
        };

        public static byte[] FromStream(string filePath,System.IO.Stream stream,StatusUpdateDelegate statusHandlers = null)
        {
            string extension = Path.GetExtension(filePath).ToLower();
            if (!VALID_EXTENSIONS.Contains(extension))
            {
                statusHandlers.Invoke(null,"BinFile.FromStream","File does not have BIN file extension.",StatusUpdateType.Error);
                return null;
            }
            byte[] block = new byte[stream.Length];
            long finalSize = stream.Read(block,0,block.Length);
            return block;
        }

        public static byte[] FromFile(string filePath,StatusUpdateDelegate statusHandlers = null)
        {
            using (var fileStream = File.OpenRead(filePath))
                return BinFile.FromStream(filePath,fileStream,statusHandlers);
        }

        public static void ToFile(string filePath,byte[] bytes)
        {
            using (var fileStream = File.Create(filePath))
                BinFile.ToStream(fileStream,bytes);
        }

        public static void ToStream(System.IO.Stream stream,byte[] bytes)
        {
            stream.Write(bytes,0,bytes.Length);
            stream.Flush();
        }
    }
}