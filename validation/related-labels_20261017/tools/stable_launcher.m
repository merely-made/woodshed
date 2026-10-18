#import <Foundation/Foundation.h>
#include <unistd.h>
#include <stdio.h>
#include <fcntl.h>
#include <stdlib.h>
#include <errno.h>
#include <sys/wait.h>
#include <libproc.h>
#include <CommonCrypto/CommonDigest.h>
static NSString *digest(NSString *path) {
 NSData *data=[NSData dataWithContentsOfFile:path options:NSDataReadingMappedIfSafe error:nil];
 if(!data)return nil; unsigned char bytes[CC_SHA256_DIGEST_LENGTH];CC_SHA256(data.bytes,(CC_LONG)data.length,bytes);
 NSMutableString *s=[NSMutableString string];for(int i=0;i<CC_SHA256_DIGEST_LENGTH;i++)[s appendFormat:@"%02x",bytes[i]];return s;
}
static BOOL save(NSString *path,id object) {
 NSData *data=[NSJSONSerialization dataWithJSONObject:object options:NSJSONWritingPrettyPrinted error:nil];
 if(!data)return NO;int f=open(path.UTF8String,O_WRONLY|O_CREAT|O_EXCL,0600);if(f<0)return NO;
 const char *p=data.bytes;size_t left=data.length;while(left){ssize_t n=write(f,p,left);if(n<0){if(errno==EINTR)continue;close(f);return NO;}p+=n;left-=n;}return close(f)==0;
}
int main(void) { @autoreleasepool {
 NSBundle *bundle=NSBundle.mainBundle;NSDictionary *info=bundle.infoDictionary;NSDictionary *env=info[@"WoodshedAcceptanceEnvironment"];
 if(!env || chdir([info[@"WoodshedAcceptanceWorkingDirectory"] UTF8String]))return 120;
 for(NSString *key in env)if(setenv(key.UTF8String,[env[key] UTF8String],1))return 121;
 NSString *dir=env[@"WOODSHED_CAPTURE_DIR"];if(!dir)return 122;
 int once=open([[dir stringByAppendingPathComponent:@"launch.once"] UTF8String],O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 123;close(once);
 NSString *program=[bundle.bundlePath stringByAppendingPathComponent:@"Contents/MacOS/woodshed"];
 NSString *binaryHash=digest(program),*scenarioHash=digest(env[@"WOODSHED_SCENARIO"]);
 if(![binaryHash isEqualToString:info[@"WoodshedAcceptanceBinarySHA256"]] || ![scenarioHash isEqualToString:info[@"WoodshedAcceptanceScenarioSHA256"]])return 124;
 int log=open([[dir stringByAppendingPathComponent:@"native.log"] UTF8String],O_WRONLY|O_CREAT|O_EXCL,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 125;
 pid_t child=fork();if(child<0)return 126;
 if(child==0){char *args[]={(char*)program.UTF8String,NULL};execv(args[0],args);_exit(127);}
 NSString *resolved=nil;char path[PROC_PIDPATHINFO_MAXSIZE];for(int i=0;i<5000;i++){int n=proc_pidpath(child,path,sizeof(path));if(n>0){NSString *s=[NSString stringWithUTF8String:path];if([s isEqualToString:program]){resolved=s;break;}}usleep(1000);}
 FILE *pid=fopen([[dir stringByAppendingPathComponent:@"native.pid"] UTF8String],"wx");if(pid){fprintf(pid,"%d\n",child);fclose(pid);}
 FILE *parent=fopen([[dir stringByAppendingPathComponent:@"supervisor.pid"] UTF8String],"wx");if(parent){fprintf(parent,"%d\n",getpid());fclose(parent);}
 NSDictionary *identity=@{@"child_pid":@(child),@"supervisor_pid":@(getpid()),@"expected_executable":program,@"observed_executable":resolved?:@"unobserved",@"executable_identity_proven":@(resolved!=nil),@"binary_sha256":binaryHash,@"scenario_sha256":scenarioHash,@"source_revision":info[@"WoodshedAcceptanceSourceRevision"]};
 if(!save([dir stringByAppendingPathComponent:@"process-identity.json"],identity))return 128;
 int status;while(waitpid(child,&status,0)<0){if(errno!=EINTR)return 129;}
 BOOL normal=WIFEXITED(status);int code=normal?WEXITSTATUS(status):-1;
 NSMutableDictionary *result=[identity mutableCopy];result[@"normal_exit"]=@(normal);result[@"exit_code"]=@(code);result[@"signal"]=@(WIFSIGNALED(status)?WTERMSIG(status):0);result[@"binary_unchanged"]=@([binaryHash isEqualToString:digest(program)]);result[@"scenario_unchanged"]=@([scenarioHash isEqualToString:digest(env[@"WOODSHED_SCENARIO"])]);
 if(!save([dir stringByAppendingPathComponent:@"exit.json"],result))return 130;
 return normal?code:131;
} }
