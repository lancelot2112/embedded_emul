using System.IO;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace EmbedEmul
{
   public abstract class FileBase
   {
      internal FileInfo _fileInfo;
      internal DateTime _lastWriteTime;
      internal TrustLevel _trustLevel;
      public string FilePath { get { return _fileInfo.FullName; } }
      public string Name { get { return _fileInfo.Name; } }
      public FileInfo FileInfo { get { return _fileInfo; } }
      public TrustLevel TrustLevel { get { return _trustLevel; } }
      public DateTime MemoryLastFileWriteTime { get { return _lastWriteTime; } }
      public event StatusUpdateDelegate StatusUpdate;
      public void OnStatusUpdate(object owner,string name,string message,StatusUpdateType type)
      {
         StatusUpdateLog.Update(owner,name,message,type);
         if (StatusUpdate != null)
            StatusUpdate(owner,name,message,type);
      }

      public event EventHandler FileSaved;
      public void OnFileSaved()
      {
         if (FileSaved != null)
            FileSaved(this,null);
      }
   }

   public enum TrustLevel
   {
      Error = 0, //Error implies parse was not successful and using the class after this point could be damaging
      Warning, //Warning means parse was successful but to be careful as data integrity may be violated
      Full
   }
}
