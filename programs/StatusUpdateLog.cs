using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.IO;

namespace EmbedEmul
{
    public enum StatusUpdateType
    {
        Output,
        Verbose,
        Warning,
        Error,
    }

    public delegate void StatusUpdateDelegate(object owner, string functionName, string message, StatusUpdateType type);
    public delegate void OpenProgressBar(string name, UInt64 total, string parent = "");
    public delegate void ReportProgress(string name, UInt64 prog, string msg, string parent = "");
    public delegate void CloseProgressBar(string name, string parent = "");

    public static class StatusUpdateLog
    {
        public static StatusUpdateType LogLevel = StatusUpdateType.Warning;
        public static string LogPath;


        public static event ReportProgress ReportProg;
        public static event OpenProgressBar StartProgBar;
        public static event CloseProgressBar CloseProgBar;

        static StatusUpdateLog()
        {
            LogPath = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData) + "\\EmbedEmul\\EmbedEmul_log";
            if (!Directory.Exists(Path.GetDirectoryName(LogPath)))
                Directory.CreateDirectory(Path.GetDirectoryName(LogPath));
        }

        public static void OnStartProgress(string name, UInt64 total, string parent ="")
        {
            OpenProgressBar prog = StartProgBar;
            if (prog != null)
               prog(name, total, parent);

        }

        public static void OnReportProgress(string name, UInt64 change, string msg, string parent ="")
        {
            ReportProgress prog = ReportProg;
            if (prog != null)
               prog(name, change, msg, parent);

        }

        public static void OnCloseProgress(string name, string parent="")
        {
            CloseProgressBar prog = CloseProgBar;
            if (prog != null)
                prog(name, parent);
        }

        public static void Update(object sender, string functionName, string message, StatusUpdateType type)
        {
            if(type >= LogLevel)
            {
                using (var file = System.IO.File.Open(LogPath, System.IO.FileMode.Append))
                using (var writer = new System.IO.StreamWriter(file))
                    writer.WriteLine(string.Format("<{0}> [{1}] {2} - {3} - {4}", type, DateTime.Now, sender, functionName, message));
            }
        }
    }
}
