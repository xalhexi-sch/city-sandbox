Add-Type @"
using System;
using System.Runtime.InteropServices;

public class Launcher {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct STARTUPINFO {
        public int cb;
        public string lpReserved;
        public string lpDesktop;
        public string lpTitle;
        public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
        public short wShowWindow, cbReserved2;
        public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct PROCESS_INFORMATION {
        public IntPtr hProcess, hThread;
        public int dwProcessId, dwThreadId;
    }
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern bool CreateProcess(
        string lpApplicationName, string lpCommandLine, IntPtr lpPA, IntPtr lpTA,
        bool bInheritHandles, uint dwCreationFlags, IntPtr lpEnv, string lpCurrentDir,
        ref STARTUPINFO lpSI, out PROCESS_INFORMATION lpPI
    );

    public static int Launch(string exe, string dir) {
        var si = new STARTUPINFO();
        si.cb = Marshal.SizeOf(si);
        si.lpDesktop = @"WinSta0\Default";
        si.dwFlags = 1; // STARTF_USESHOWWINDOW
        si.wShowWindow = 1; // SW_SHOWNORMAL
        var pi = new PROCESS_INFORMATION();
        string cmd = "\"" + exe + "\"";
        bool ok = CreateProcess(null, cmd, IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, dir, ref si, out pi);
        if (!ok) throw new Exception("Error: " + Marshal.GetLastWin32Error());
        return pi.dwProcessId;
    }
}
"@

$exe = "C:\Users\mayke\Downloads\city-sandbox\city-sandbox\target\debug\city-sandbox.exe"
$dir = "C:\Users\mayke\Downloads\city-sandbox\city-sandbox"
$pidVal = [Launcher]::Launch($exe, $dir)
Write-Output "Launched city-sandbox with PID $pidVal on user desktop (WinSta0\Default)"
Wait-Process -Id $pidVal
