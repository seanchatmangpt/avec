/* examples/c99_consumer/main.c — hand-written C99 consumer (NOT generated).
 *
 * REL-L2 smoke (v26.10.29): proves the ggen-generated avec headers serve a
 * real freestanding C consumer with no avec library to link — the headers
 * are the whole product. Build: see scripts/c99_smoke.sh.
 *
 * Compile-time layout law (Apache Arrow C Data Interface ABI, LP64):
 *   sizeof(struct ArrowArray)  == 80  (5 x int64 + 5 pointers)
 *   sizeof(struct ArrowSchema) == 72  (3 char* + 2 x int64 + 4 pointers)
 * and for both structs the trailing fields are release (function pointer)
 * then private_data (void*), private_data being the last field.
 *
 * Note: the lane contract said both structs are 80 bytes on LP64. Per the
 * Arrow C Data Interface and per this header, ArrowSchema is 72 bytes on
 * LP64 (9 pointer-sized members); an ==80 assert for ArrowSchema does not
 * compile. The real ABI values are asserted here.
 *
 * Static asserts use the C99 negative-array-size idiom so the build stays
 * warning-clean under -std=c99 -pedantic (C11 _Static_assert would warn). */

#include <stddef.h> /* offsetof, size_t — ISO C freestanding header */
#include <stdio.h>  /* printf, snprintf, puts — hosted runtime at link */
#include <string.h> /* strcmp */
#include <inttypes.h>

#include "avec_c_data.h"

/* Double inclusion is deliberate: avec_format_codes.h's include guard is
 * mismatched (#ifndef AVEC_FORMAT_CODES_H vs #define AVEC_FORMAT_CODES_C_H),
 * so idempotence rests on C99 6.10.3p2 (identical macro redefinition is
 * legal) — this TU compiles and runs with the file included twice. Known
 * diagnostic, external to this lane: clang's -Wheader-guard fires on that
 * header even at single inclusion (verified with a one-include TU); it is
 * the only diagnostic in this build and originates in include/, not here. */
#include "avec_format_codes.h"
#include "avec_format_codes.h"

/* --- C99 compile-time assert (negative array size) ----------------------- */

#define AVEC_STATIC_ASSERT(cond, tag)                                          \
  typedef char avec_static_assert_##tag[(cond) ? 1 : -1]

/* Exact offsets/sizes below hold only when pointers are 8 bytes (LP64). */
AVEC_STATIC_ASSERT(sizeof(void *) == 8, lp64_pointers_are_8_bytes);

/* struct ArrowArray: 5 x int64 (0..39) then 5 pointers (40..79) = 80. */
AVEC_STATIC_ASSERT(sizeof(struct ArrowArray) == 80, arrow_array_size_80_lp64);
AVEC_STATIC_ASSERT(offsetof(struct ArrowArray, length) == 0, array_length_first);
AVEC_STATIC_ASSERT(offsetof(struct ArrowArray, buffers) == 40, array_buffers_40);
AVEC_STATIC_ASSERT(offsetof(struct ArrowArray, release) == 64, array_release_64);
AVEC_STATIC_ASSERT(offsetof(struct ArrowArray, private_data) == 72,
                   array_private_data_72);
AVEC_STATIC_ASSERT(offsetof(struct ArrowArray, release) <
                       offsetof(struct ArrowArray, private_data),
                   array_release_precedes_private_data);
AVEC_STATIC_ASSERT(offsetof(struct ArrowArray, private_data) + sizeof(void *) ==
                       sizeof(struct ArrowArray),
                   array_private_data_is_last_field);

/* struct ArrowSchema: 3 char* (0..23) + 2 x int64 (24..39) + 4 pointers
 * (40..71) = 72 on LP64 (real ABI value; see file comment). */
AVEC_STATIC_ASSERT(sizeof(struct ArrowSchema) == 72, arrow_schema_size_72_lp64);
AVEC_STATIC_ASSERT(offsetof(struct ArrowSchema, format) == 0, schema_format_first);
AVEC_STATIC_ASSERT(offsetof(struct ArrowSchema, flags) == 24, schema_flags_24);
AVEC_STATIC_ASSERT(offsetof(struct ArrowSchema, release) == 56, schema_release_56);
AVEC_STATIC_ASSERT(offsetof(struct ArrowSchema, private_data) == 64,
                   schema_private_data_64);
AVEC_STATIC_ASSERT(offsetof(struct ArrowSchema, release) <
                       offsetof(struct ArrowSchema, private_data),
                   schema_release_precedes_private_data);
AVEC_STATIC_ASSERT(offsetof(struct ArrowSchema, private_data) + sizeof(void *) ==
                       sizeof(struct ArrowSchema),
                   schema_private_data_is_last_field);

/* Flag macro values are part of the schema-flags ABI. */
AVEC_STATIC_ASSERT(ARROW_FLAG_DICTIONARY_ORDERED == 1, flag_ordered_is_1);
AVEC_STATIC_ASSERT(ARROW_FLAG_NULLABLE == 2, flag_nullable_is_2);
AVEC_STATIC_ASSERT(ARROW_FLAG_MAP_KEYS_SORTED == 4, flag_map_sorted_is_4);

/* --- runtime checks on real state ---------------------------------------- */

static int avec_failures = 0;

#define CHECK(cond)                                                            \
  do {                                                                         \
    if (!(cond)) {                                                             \
      avec_failures++;                                                         \
      printf("FAIL: %s\n", #cond);                                             \
    }                                                                          \
  } while (0)

static void print_layout(void) {
  printf("avec c99 smoke v26.10.29\n");
  printf("sizeof(void*)                = %zu\n", sizeof(void *));
  printf("sizeof(struct ArrowArray)    = %zu\n", sizeof(struct ArrowArray));
  printf("  offsetof release           = %zu\n",
         offsetof(struct ArrowArray, release));
  printf("  offsetof private_data      = %zu\n",
         offsetof(struct ArrowArray, private_data));
  printf("sizeof(struct ArrowSchema)   = %zu\n", sizeof(struct ArrowSchema));
  printf("  offsetof flags             = %zu\n",
         offsetof(struct ArrowSchema, flags));
  printf("  offsetof release           = %zu\n",
         offsetof(struct ArrowSchema, release));
  printf("  offsetof private_data      = %zu\n",
         offsetof(struct ArrowSchema, private_data));
}

int main(void) {
  const char *fmt_list_int32 = AVEC_FMT_LIST AVEC_FMT_INT32; /* "+li" */
  const char *fmt_int32 = AVEC_FMT_INT32; /* "i" */
  char fmt_ts_micro[32];
  int64_t flags = 0;
  struct ArrowArray arr;
  struct ArrowSchema schema;

  /* Parameterized prefix form documented in the generated header:
   * TimestampMicro is `tsu:{...}` (no macro is generated for prefixes). */
  (void)snprintf(fmt_ts_micro, sizeof(fmt_ts_micro), "tsu:%s", "UTC");

  /* Flags ORed into an int64 via the generated macros. */
  flags |= ARROW_FLAG_NULLABLE;
  flags |= ARROW_FLAG_DICTIONARY_ORDERED;
  flags |= ARROW_FLAG_MAP_KEYS_SORTED;

  /* A real ArrowArray + ArrowSchema instance, wired header-only. */
  arr.length = 3;
  arr.null_count = 0;
  arr.offset = 0;
  arr.n_buffers = 3;
  arr.n_children = 1;
  arr.buffers = 0;
  arr.children = 0;
  arr.dictionary = 0;
  arr.release = 0;
  arr.private_data = 0;

  schema.format = fmt_list_int32;
  schema.name = "a_list_of_int32";
  schema.metadata = 0;
  schema.flags = flags;
  schema.n_children = 1;
  schema.children = 0;
  schema.dictionary = 0;
  schema.release = 0;
  schema.private_data = 0;

  print_layout();

  /* Assert final state of the real instances (Chicago: state, not calls). */
  CHECK(sizeof(struct ArrowArray) == 80);
  CHECK(sizeof(struct ArrowSchema) == 72);
  CHECK(offsetof(struct ArrowArray, private_data) == 72);
  CHECK(offsetof(struct ArrowSchema, private_data) == 64);
  CHECK(flags == (ARROW_FLAG_DICTIONARY_ORDERED | ARROW_FLAG_NULLABLE |
                  ARROW_FLAG_MAP_KEYS_SORTED));
  CHECK(flags == 7);
  CHECK(schema.flags == 7);
  CHECK(arr.length == 3 && arr.n_buffers == 3 && arr.null_count == 0);
  CHECK(schema.format == fmt_list_int32);
  CHECK(strcmp(fmt_int32, "i") == 0);
  CHECK(strcmp(fmt_list_int32, "+li") == 0);
  CHECK(strcmp(fmt_ts_micro, "tsu:UTC") == 0);
  CHECK(AVEC_FMT_INT64[0] == 'l');
  CHECK(AVEC_FMT_BOOLEAN[0] == 'b');

  if (avec_failures == 0) {
    printf("format: %s (list of %s), ts=%s, flags=%" PRId64 "\n",
           schema.format, fmt_int32, fmt_ts_micro, schema.flags);
    puts("c99 smoke: ALL CHECKS PASSED");
  } else {
    printf("c99 smoke: %d CHECK(S) FAILED\n", avec_failures);
  }
  return avec_failures == 0 ? 0 : 1;
}
