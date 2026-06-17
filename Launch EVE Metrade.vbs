Option Explicit

Dim fso, shell, folder, cmdPath, logPath, command, exitCode

Set fso = CreateObject("Scripting.FileSystemObject")
Set shell = CreateObject("WScript.Shell")

folder = fso.GetParentFolderName(WScript.ScriptFullName)
cmdPath = folder & "\Launch EVE Metrade.cmd"
logPath = folder & "\launcher.log"

shell.Environment("PROCESS")("EVE_METRADE_NO_PAUSE") = "1"
command = "cmd /c call """ & cmdPath & """ > """ & logPath & """ 2>&1"
exitCode = shell.Run(command, 0, True)

If exitCode <> 0 Then
  MsgBox "EVE Metrade failed to build or launch. See launcher.log in the app folder.", vbExclamation, "EVE Metrade"
End If
