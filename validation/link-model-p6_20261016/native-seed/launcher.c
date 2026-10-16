#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <fcntl.h>
int main(void) {
if (chdir("/Users/markik/Code/repos/woodshed-link-model-p3-20261016") != 0) return 120;
if (setenv("WOODSHED_PRESENTATION_TIMEOUT_SECS", "60", 1) != 0) return 121;
if (setenv("WOODSHED_WIDTH", "1100", 1) != 0) return 121;
if (setenv("WOODSHED_HEIGHT", "900", 1) != 0) return 121;
if (setenv("WOODSHED_STATE", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/profile/practice.bin", 1) != 0) return 121;
if (setenv("WOODSHED_SETTINGS", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/profile/settings.json", 1) != 0) return 121;
if (setenv("GRAPHSHELL_APP_ENDPOINT", "/tmp/ws-link-p6-20261016-a.sock", 1) != 0) return 121;
if (setenv("TMPDIR", "/tmp", 1) != 0) return 121;
if (setenv("XDG_DATA_HOME", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/xdg-data", 1) != 0) return 121;
if (setenv("XDG_CONFIG_HOME", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/xdg-config", 1) != 0) return 121;
if (setenv("XDG_CACHE_HOME", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/xdg-cache", 1) != 0) return 121;
if (setenv("MERE_ROOT", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/mere-root", 1) != 0) return 121;
if (setenv("WOODSHED_SCENARIO", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/scenarios/p6f_relationship_link_rest_seed.scn", 1) != 0) return 121;
if (setenv("WOODSHED_CAPTURE_DIR", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/native-seed-v1", 1) != 0) return 121;
if (setenv("WOODSHED_RECEIPT", "/Users/markik/Code/testing/woodshed/link-model-p6-20261016/native-seed-v1/scenario.done", 1) != 0) return 121;
FILE *pid = fopen("/Users/markik/Code/testing/woodshed/link-model-p6-20261016/native-seed-v1/native.pid", "w");
if (pid) { fprintf(pid, "%d\n", getpid()); fclose(pid); }
int log = open("/Users/markik/Code/testing/woodshed/link-model-p6-20261016/native-seed-v1/native.log", O_WRONLY | O_CREAT | O_TRUNC, 0600);
if (log < 0 || dup2(log, 1) < 0 || dup2(log, 2) < 0) return 122;
char *args[] = {"/Users/markik/Code/testing/woodshed/link-model-p6-20261016/frozen-d896e39/woodshed-genet", NULL};
execv(args[0], args);
perror("execv");
return 123;
}
