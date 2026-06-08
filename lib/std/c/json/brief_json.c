/*
 * brief_json.c — Brief JSON FFI bridge using yyjson
 *
 * Provides C-callable implementations of the JSON functions
 * declared in lib/std/json.bv.
 *
 * Included by: import "link/json/brief_json.c" in Brief source.
 */
#include "yyjson.h"
#include <string.h>
#include <stdint.h>

/* Parse a JSON string into a yyjson_mut_doc.
 * Returns a pointer as i64 (null on parse error).
 * Caller must free with json_free. */
int64_t brief_json_parse(const char* data, uint64_t len) {
    yyjson_doc* doc = yyjson_read(data, len, 0);
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    if (!root) { yyjson_doc_free(doc); return 0; }
    return (int64_t)(intptr_t)doc;
}

/* Free a previously parsed JSON document. */
void brief_json_free(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (doc) yyjson_doc_free(doc);
}

/* Check if the root value is an object. Returns 1 if object, 0 otherwise. */
int brief_json_is_object(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    return root && yyjson_is_obj(root) ? 1 : 0;
}

/* Check if the root value is an array. */
int brief_json_is_array(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    return root && yyjson_is_arr(root) ? 1 : 0;
}

/* Check if the root value is a string. */
int brief_json_is_string(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    return root && yyjson_is_str(root) ? 1 : 0;
}

/* Check if the root value is a number. */
int brief_json_is_number(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    return root && yyjson_is_num(root) ? 1 : 0;
}

/* Check if the root value is a boolean. */
int brief_json_is_bool(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    return root && yyjson_is_bool(root) ? 1 : 0;
}

/* Check if the root value is null. */
int brief_json_is_null(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    return root && yyjson_is_null(root) ? 1 : 0;
}

/* Get a string value by key from a JSON object root.
 * Returns the string pointer (null if not found or not string).
 * Sets len to the string length. */
const char* brief_json_get_string(int64_t handle, const char* key, uint64_t* len) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    if (!root || !yyjson_is_obj(root)) return 0;
    yyjson_val* val = yyjson_obj_get(root, key);
    if (!val) return 0;
    return yyjson_get_str(val);
}

/* Get an integer value by key from a JSON object root. */
int64_t brief_json_get_int(int64_t handle, const char* key) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    if (!root || !yyjson_is_obj(root)) return 0;
    yyjson_val* val = yyjson_obj_get(root, key);
    if (!val) return 0;
    return yyjson_get_int(val);
}

/* Stringify a JSON document back to a string.
 * Returns a malloc'd buffer. Caller must brief_json_free_str. */
char* brief_json_stringify(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    if (!root) return 0;
    return yyjson_val_write(root, 0, NULL);
}

/* Free a string returned by brief_json_stringify. */
void brief_json_free_str(char* s) {
    if (s) free(s);
}

/* Get the number of elements (array length or object size). */
uint64_t brief_json_get_len(int64_t handle) {
    yyjson_doc* doc = (yyjson_doc*)(intptr_t)handle;
    if (!doc) return 0;
    yyjson_val* root = yyjson_doc_get_root(doc);
    if (!root) return 0;
    if (yyjson_is_arr(root)) return yyjson_arr_size(root);
    if (yyjson_is_obj(root)) return yyjson_obj_size(root);
    return 0;
}
