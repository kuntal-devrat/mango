// WPT test harness report integration for Mango Browser.
//
// The upstream `resources/testharnessreport.js` is designed for wptrunner's
// JSON protocol. Mango drives the harness directly, so this shim publishes the
// same information through two console markers:
//
//   __WPT_COMPLETE__:<json>  emitted once, when the harness completes
//   __WPT_SUMMARY__:<json>   emitted on demand, for the final summary
//
// The payload shape mirrors wptrunner's expectations:
//   { status, message, tests: [{ name, status, message, stack }], passed, total }
//
// `status` is the upstream `TestsStatus` enum:
//   0 = OK, 1 = ERROR, 2 = TIMEOUT, 3 = PRECONDITION_FAILED
(function() {
    if (typeof setup === 'function') {
        setup({ output: false });
    }
    var accumulated = [];

    function statusToString(status) {
        if (status === 0) return "PASS";
        if (status === 1) return "FAIL";
        if (status === 2) return "TIMEOUT";
        if (status === 3) return "NOTRUN";
        if (status === 4) return "PRECONDITION_FAILED";
        return "UNKNOWN";
    }

    function buildResults(harnessStatus) {
        var passed = 0;
        var list = [];
        for (var i = 0; i < accumulated.length; i++) {
            var t = accumulated[i];
            var s = statusToString(t.status);
            if (s === "PASS") passed++;
            list.push({
                name: t.name,
                status: s,
                message: t.message || null,
                stack: t.stack || null
            });
        }
        return {
            status: harnessStatus ? harnessStatus.status : 0,
            message: harnessStatus ? harnessStatus.message : null,
            tests: list,
            passed: passed,
            total: list.length
        };
    }

    if (typeof add_result_callback === 'function') {
        add_result_callback(function(t) {
            accumulated.push(t);
        });
    }

    if (typeof add_completion_callback === 'function') {
        add_completion_callback(function(tests, harnessStatus) {
            if (tests && tests.length) accumulated = tests;
            var results = buildResults(harnessStatus);
            globalThis.__wpt_results = results;
            if (typeof console !== 'undefined' && console.log) {
                console.log("__WPT_COMPLETE__:" + JSON.stringify(results));
            }
        });
    }
})();
