param([int]$AppPid,[ValidateSet('inspect','cancel','save','close')][string]$Action,[string]$Destination,[string]$Capture)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class DiagnosticWindows {
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h,EnumProc cb,IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageTimeoutW")] public static extern IntPtr ReadText(IntPtr h,uint m,UIntPtr w,StringBuilder s,uint f,uint ms,out UIntPtr result);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageTimeoutW")] public static extern IntPtr WriteText(IntPtr h,uint m,UIntPtr w,string s,uint f,uint ms,out UIntPtr result);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageTimeoutW")] public static extern IntPtr NumericMessage(IntPtr h,uint m,UIntPtr w,IntPtr l,uint f,uint ms,out UIntPtr result);
 public static bool TypeFileName(IntPtr h,string text){UIntPtr result;if(NumericMessage(h,0xb1,UIntPtr.Zero,(IntPtr)(-1),2,1500,out result)==IntPtr.Zero)return false;foreach(char c in text){if(NumericMessage(h,0x102,(UIntPtr)(uint)c,(IntPtr)1,2,1500,out result)==IntPtr.Zero)return false;}return true;}
 public static string ControlText(IntPtr h){var s=new StringBuilder(2048);UIntPtr result;ReadText(h,0xd,(UIntPtr)2048,s,2,1500,out result);return s.ToString();}

 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint f);
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[DiagnosticWindows]::SetProcessDPIAware() | Out-Null
$deadline=[DateTime]::UtcNow.AddSeconds(10)
do {
$script:owned=New-Object 'System.Collections.Generic.List[object]'
[DiagnosticWindows]::EnumWindows({param($h,$l)
 [uint32]$owner=0;[DiagnosticWindows]::GetWindowThreadProcessId($h,[ref]$owner)|Out-Null
 if($owner -eq $AppPid -and [DiagnosticWindows]::IsWindowVisible($h)){
  $s=New-Object System.Text.StringBuilder 512;[DiagnosticWindows]::GetWindowText($h,$s,512)|Out-Null
  $script:owned.Add([pscustomobject]@{handle=$h.ToInt64();title=$s.ToString()})
 };return $true
},[IntPtr]::Zero)|Out-Null
 $found=@($script:owned|Where-Object title -eq $(if($Action -eq 'close'){'Woodshed'}else{'Export MIDI diagnostics to a new file'}))
 if($found.Count -eq 1){break}
 Start-Sleep -Milliseconds 100
} while([DateTime]::UtcNow -lt $deadline)
$main=@($script:owned|Where-Object title -eq 'Woodshed')
if($Action -eq 'close'){
 if($main.Count -ne 1){throw 'Expected one owned Woodshed window'}
 if(![DiagnosticWindows]::PostMessage([IntPtr]$main[0].handle,0x10,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Native WM_CLOSE failed'}
 @{action='native-WM_CLOSE';pid=$AppPid;window=$main[0]}|ConvertTo-Json -Depth 5;exit 0
}
$panels=@($script:owned|Where-Object title -eq 'Export MIDI diagnostics to a new file')
if($panels.Count -ne 1){throw ('Expected one owned diagnostic save dialog: '+($script:owned|ConvertTo-Json -Compress))}
$h=[IntPtr]$panels[0].handle
$script:children=New-Object 'System.Collections.Generic.List[object]'
[DiagnosticWindows]::EnumChildWindows($h,{param($child,$l)
 if([DiagnosticWindows]::IsWindowVisible($child)){
  $c=New-Object System.Text.StringBuilder 128;[DiagnosticWindows]::GetClassName($child,$c,128)|Out-Null
  if($c.ToString() -in @('Button','Edit')){
   $script:children.Add([pscustomobject]@{handle=$child.ToInt64();class=$c.ToString();id=[DiagnosticWindows]::GetDlgCtrlID($child);text=[DiagnosticWindows]::ControlText($child)})
  }
 };return $true
},[IntPtr]::Zero)|Out-Null
$fields=@($script:children|Where-Object class -eq 'Edit')
$buttons=@($script:children|Where-Object class -eq 'Button')
$fileFields=@($fields|Where-Object {$_.text -in @('midi-diagnostics','midi-diagnostics.json')})
$saveButtons=@($buttons|Where-Object {$_.text.Replace('&','') -eq 'Save'})
$cancelButtons=@($buttons|Where-Object {$_.text.Replace('&','') -eq 'Cancel'})
@{phase='native-control-inventory';action=$Action;pid=$AppPid;windows=$script:owned;fields=$fields;buttons=$buttons}|ConvertTo-Json -Depth 6
if($Capture){
 $rect=New-Object DiagnosticWindows+Rect;[DiagnosticWindows]::GetWindowRect($h,[ref]$rect)|Out-Null
 $bitmap=New-Object System.Drawing.Bitmap ($rect.R-$rect.L),($rect.B-$rect.T)
 $graphics=[System.Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
 try{$printed=[DiagnosticWindows]::PrintWindow($h,$dc,2)}finally{$graphics.ReleaseHdc($dc)}
 $bitmap.Save($Capture,[System.Drawing.Imaging.ImageFormat]::Png);$graphics.Dispose();$bitmap.Dispose()
}
if($Action -eq 'save'){
 if($fileFields.Count -ne 1 -or $saveButtons.Count -ne 1){throw 'Expected one native filename Edit and Save Button'}
 # Native character entry emits the combobox edit notifications; WM_SETTEXT
 # alone can leave the shell's committed filename pointing at its prior folder.
 if(![DiagnosticWindows]::TypeFileName([IntPtr]$fileFields[0].handle,$Destination)){throw 'Native filename character entry timed out'}
 if([DiagnosticWindows]::ControlText([IntPtr]$fileFields[0].handle) -ne $Destination){throw 'Native filename edit did not stick'}
 if(![DiagnosticWindows]::PostMessage([IntPtr]$saveButtons[0].handle,0xf5,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Native Save BM_CLICK failed'}
}elseif($Action -eq 'cancel'){
 if($cancelButtons.Count -ne 1){throw 'Expected one native Cancel Button'}
 if(![DiagnosticWindows]::PostMessage([IntPtr]$cancelButtons[0].handle,0xf5,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Native Cancel BM_CLICK failed'}
}
@{phase='native-action-submitted';action=$Action;pid=$AppPid;destination=$Destination;filename_entry='EM_SETSEL and WM_CHAR';native_print=$printed}|ConvertTo-Json -Depth 6
