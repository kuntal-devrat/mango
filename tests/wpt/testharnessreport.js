// WPT test harness report integration for Mango Browser
(function() {
    var accumulated_tests = [];

    function statusToString(status) {
        if (status === 0) return "PASS";
        if (status === 1) return "FAIL";
        if (status === 2) return "TIMEOUT";
        if (status === 3) return "NOTRUN";
        if (status === 4) return "PRECONDITION_FAILED";
        return "UNKNOWN";
    }

    function updateResults(harness_status) {
        var passed = 0;
        var list = [];
        for (var i = 0; i < accumulated_tests.length; i++) {
            var t = accumulated_tests[i];
            var s = statusToString(t.status);
            if (s === "PASS") passed++;
            list.push({
                name: t.name,
                status: s,
                message: t.message || null,
                stack: t.stack || null
            });
        }
        var results = {
            status: harness_status ? harness_status.status : 0,
            message: harness_status ? harness_status.message : null,
            tests: list,
            passed: passed,
            total: list.length
        };
        globalThis.__wpt_results = results;
        return results;
    }

    if (typeof add_result_callback === 'function') {
        add_result_callback(function(t) {
            accumulated_tests.push(t);
            updateResults(null);
        });
    }

    if (typeof add_completion_callback === 'function') {
        add_completion_callback(function(tests, harness_status) {
            accumulated_tests = tests || accumulated_tests;
            var results = updateResults(harness_status);
            if (typeof console !== 'undefined' && console.log) {
                console.log("__WPT_COMPLETE__:" + JSON.stringify(results));
            }
        });
    }
})();
