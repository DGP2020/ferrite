#include <stdint.h>

/*
 * Clamps an integer value to the range [min, max].
 * Pure C algorithmic utility function.
 */
int clamp(int val, int min, int max) {
    if (val < min) {
        return min;
    } else if (val > max) {
        return max;
    } else {
        return val;
    }
}
