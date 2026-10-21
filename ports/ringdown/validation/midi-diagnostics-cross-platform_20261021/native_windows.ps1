param([int]$AppPid,[ValidateSet('inspect','cancel','save','close')][string]$Action,[string]$Destination,[string]$Capture)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes,System.Drawing
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
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint f);
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int L,T,R,B;}
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[DiagnosticWindows]::SetProcessDPIAware() | Out-Null
$script:owned=New-Object 'System.Collections.Generic.List[object]'
[DiagnosticWindows]::EnumWindows({param($h,$l)
 [uint32]$owner=0;[DiagnosticWindows]::GetWindowThreadProcessId($h,[ref]$owner)|Out-Null
 if($owner -eq $AppPid -and [DiagnosticWindows]::IsWindowVisible($h)){
  $s=New-Object System.Text.StringBuilder 512;[DiagnosticWindows]::GetWindowText($h,$s,512)|Out-Null
  $script:owned.Add([pscustomobject]@{handle=$h.ToInt64();title=$s.ToString()})
 };return $true
},[IntPtr]::Zero)|Out-Null
$main=@($script:owned|Where-Object title -eq 'Woodshed')
if($Action -eq 'close'){
 if($main.Count -ne 1){throw 'Expected one owned Woodshed window'}
 if(![DiagnosticWindows]::PostMessage([IntPtr]$main[0].handle,0x10,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Native WM_CLOSE failed'}
 @{action='native-WM_CLOSE';pid=$AppPid;window=$main[0]}|ConvertTo-Json -Depth 5;exit 0
}
$panels=@($script:owned|Where-Object title -eq 'Export MIDI diagnostics to a new file')
if($panels.Count -ne 1){throw ('Expected one owned diagnostic save dialog: '+($script:owned|ConvertTo-Json -Compress))}
$h=[IntPtr]$panels[0].handle
$root=[System.Windows.Automation.AutomationElement]::FromHandle($h)
$edits=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,(New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ControlTypeProperty),([System.Windows.Automation.ControlType]::Edit)))
$fields=@();$fileField=$null
foreach($e in $edits){
 $v=$null;try{$v=$e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value}catch{}
 $fields+=@{name=$e.Current.Name;id=$e.Current.AutomationId;value=$v}
 if($v -eq 'midi-diagnostics.json'){$fileField=$e}
}
$buttons=$root.FindAll([System.Windows.Automation.TreeScope]::Descendants,(New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ControlTypeProperty),([System.Windows.Automation.ControlType]::Button)))
$names=@();$save=$null;$cancel=$null
foreach($e in $buttons){$name=$e.Current.Name;$names+=$name;if($name -eq 'Save'){$save=$e};if($name -eq 'Cancel'){$cancel=$e}}
if($Capture){
 $rect=New-Object DiagnosticWindows+Rect;[DiagnosticWindows]::GetWindowRect($h,[ref]$rect)|Out-Null
 $bitmap=New-Object System.Drawing.Bitmap ($rect.R-$rect.L),($rect.B-$rect.T)
 $graphics=[System.Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
 try{$printed=[DiagnosticWindows]::PrintWindow($h,$dc,2)}finally{$graphics.ReleaseHdc($dc)}
 $bitmap.Save($Capture,[System.Drawing.Imaging.ImageFormat]::Png);$graphics.Dispose();$bitmap.Dispose()
}
if($Action -eq 'save'){
 if(!$fileField -or !$save){throw ('Native File name / Save control missing: '+($fields|ConvertTo-Json -Compress))}
 $fileField.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Destination)
 $save.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}elseif($Action -eq 'cancel'){
 if(!$cancel){throw 'Native Cancel control missing'}
 $cancel.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}
@{action=$Action;pid=$AppPid;windows=$script:owned;fields=$fields;buttons=$names;native_print=$printed}|ConvertTo-Json -Depth 6
