// Python C extension: brief_bridge (auto-generated)
#include <Python.h>
#include <dlfcn.h>
#include <stdint.h>

static void* lib_handle = NULL;

static long long carg_0;
static long long carg_1;

typedef long long (*add_fn_t)(long long, long long);
static add_fn_t add_fn = NULL;

static PyObject* pybridge_add(PyObject* self, PyObject* args) {
    (void)self;
    if (!PyArg_ParseTuple(args, "LL", &carg_0, &carg_1)) return NULL;
    long long result = add_fn(carg_0, carg_1);
    return PyLong_FromLongLong(result);
}
static long long carg_0;
static long long carg_1;

typedef long long (*mul_fn_t)(long long, long long);
static mul_fn_t mul_fn = NULL;

static PyObject* pybridge_mul(PyObject* self, PyObject* args) {
    (void)self;
    if (!PyArg_ParseTuple(args, "LL", &carg_0, &carg_1)) return NULL;
    long long result = mul_fn(carg_0, carg_1);
    return PyLong_FromLongLong(result);
}

static PyMethodDef methods[] = {
    {"add", pybridge_add, METH_VARARGS, "Brief export add"},
    {"mul", pybridge_mul, METH_VARARGS, "Brief export mul"},
    {NULL, NULL, 0, NULL}
};

static struct PyModuleDef module_def = {
    PyModuleDef_HEAD_INIT,
    "brief_bridge",
    "Brief bridge (Metropolitan FFI)",
    0,
    methods
};

PyMODINIT_FUNC PyInit_brief_bridge(void) {
    lib_handle = dlopen("./out/bench_add.so", RTLD_LAZY | RTLD_LOCAL);
    if (!lib_handle) { PyErr_SetString(PyExc_RuntimeError, dlerror()); return NULL; }

    add_fn = (add_fn_t)dlsym(lib_handle, "add");
    if (!add_fn) { PyErr_SetString(PyExc_RuntimeError, "dlsym add failed"); return NULL; }
    mul_fn = (mul_fn_t)dlsym(lib_handle, "mul");
    if (!mul_fn) { PyErr_SetString(PyExc_RuntimeError, "dlsym mul failed"); return NULL; }
    return PyModuleDef_Init(&module_def);
}
