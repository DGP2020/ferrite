#include <stdint.h>
#include <stddef.h>

size_t cstr_len_bounded(const char *s) {
    size_t len = 0;
    while (s[len] != '\0' && len < 4096) {
        len++;
    }
    return len;
}

size_t count_char_ascii(const char *s) {
    size_t count = 0;
    for (size_t i = 0; s[i] != '\0'; i++) {
        if (s[i] == 'a' || s[i] == 'A') {
            count++;
        }
    }
    return count;
}

int is_ascii_digits(const char *s) {
    if (s[0] == '\0') return 0;
    for (size_t i = 0; s[i] != '\0'; i++) {
        if (s[i] < '0' || s[i] > '9') {
            return 0;
        }
    }
    return 1;
}
