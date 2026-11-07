// Keep the application source control above both views, so switching does not shift it.
(() => {
  const { streamDeckClient } = SDPIComponents;
  const modeSelect = document.querySelector('sdpi-select[setting="mode"]');
  const sourceSelect = document.querySelector(
    'sdpi-select[setting="application_source"]',
  );
  const applicationSelect = document.querySelector(
    'sdpi-select[setting="application"]',
  );
  const outputField = document.getElementById("output-field");
  const sourceField = document.getElementById("application-source-field");
  const applicationField = document.getElementById("application-field");
  const convertField = document.getElementById("application-convert-field");
  const convertButton = document.getElementById("convert-application");
  const convertResult = document.getElementById("convert-result");
  const customFields = [
    "custom-field",
    "custom-pattern-field",
    "custom-case-field",
    "custom-name-field",
    "custom-help-field",
  ].map((id) => document.getElementById(id));
  const insetField = document.getElementById("inset-field");
  const programField = document.getElementById("program-field");
  let currentMode = "focused_window";
  let currentSource = "installed";
  let selectedApplication = "";
  let converting = false;
  let settingsSettleAt = 0;

  function markPendingSave() {
    // SDPI controls debounce settings writes for 200 ms.
    settingsSettleAt = Date.now() + 250;
  }

  function updateVisibility() {
    const application = currentMode === "defined_application";
    const custom = application && currentSource === "custom";
    outputField.style.display = currentMode === "specific_output" ? "" : "none";
    sourceField.style.display = application ? "" : "none";
    applicationField.style.display = application && !custom ? "" : "none";
    convertField.style.display = application && !custom ? "" : "none";
    convertButton.disabled = converting || !selectedApplication;
    for (const field of customFields)
      field.style.display = custom ? "" : "none";
    insetField.style.display = currentMode === "focused_window" ? "" : "none";
    programField.style.display =
      currentMode === "programmed_window" ? "" : "none";
  }

  function updateSettings(settings) {
    currentMode = settings.mode ?? "focused_window";
    currentSource = settings.application_source ?? "installed";
    selectedApplication = settings.application ?? "";
    updateVisibility();
  }

  modeSelect.addEventListener("valuechange", () => {
    currentMode = modeSelect.value;
    markPendingSave();
    updateVisibility();
  });
  sourceSelect.addEventListener("valuechange", () => {
    currentSource = sourceSelect.value;
    markPendingSave();
    updateVisibility();
  });
  applicationSelect.addEventListener("valuechange", () => {
    selectedApplication = applicationSelect.value;
    markPendingSave();
    updateVisibility();
  });

  convertButton.addEventListener("click", async () => {
    if (
      converting ||
      !selectedApplication ||
      currentMode !== "defined_application" ||
      currentSource !== "installed"
    )
      return;
    const selection = selectedApplication;
    const stillSelected = () =>
      currentMode === "defined_application" &&
      currentSource === "installed" &&
      selectedApplication === selection;
    converting = true;
    convertResult.textContent = "Resolving application identity…";
    updateVisibility();
    try {
      const response = await streamDeckClient.get(
        "sendToPlugin",
        "sendToPropertyInspector",
        (message) => message.payload?.event === "getApplicationConversion",
        { event: "getApplicationConversion", application: selection },
      );
      if (!stillSelected()) {
        convertResult.textContent = "Selection changed; conversion cancelled.";
        return;
      }
      const rule = response.payload?.conversion;
      if (!rule?.pattern) {
        convertResult.textContent =
          "Unable to identify the selected application.";
        return;
      }
      // Do not race the selector's pending settings write.
      const remaining = settingsSettleAt - Date.now();
      if (remaining > 0)
        await new Promise((resolve) => setTimeout(resolve, remaining));
      if (!stillSelected()) {
        convertResult.textContent = "Selection changed; conversion cancelled.";
        return;
      }
      const { settings } = await streamDeckClient.getSettings();
      if (
        !stillSelected() ||
        (settings.mode ?? "focused_window") !== "defined_application" ||
        (settings.application_source ?? "installed") !== "installed"
      ) {
        convertResult.textContent = "Selection changed; conversion cancelled.";
        return;
      }
      const updated = {
        ...settings,
        application: selection,
        application_source: "custom",
        custom_field: "app_id",
        custom_pattern: rule.pattern,
        custom_case_sensitive: true,
        custom_name: (rule.name || rule.pattern).slice(0, 24),
      };
      await streamDeckClient.setSettings(updated);
      // Refresh the SDPI components from the saved settings. Setting the
      // select value manually here schedules a separate settings write
      // and can restore the previous custom rule from SDPI's old cache.
      const { settings: saved } = await streamDeckClient.getSettings();
      if (
        saved.application_source !== "custom" ||
        saved.custom_field !== updated.custom_field ||
        saved.custom_pattern !== updated.custom_pattern ||
        saved.custom_case_sensitive !== updated.custom_case_sensitive ||
        saved.custom_name !== updated.custom_name
      ) {
        convertResult.textContent =
          "Unable to confirm the saved matching rule.";
        return;
      }
      updateSettings(saved);
      convertResult.textContent = "Converted to an editable matching rule.";
    } catch (error) {
      console.error("Application conversion failed:", error);
      convertResult.textContent = "Unable to convert the selected application.";
    } finally {
      converting = false;
      updateVisibility();
    }
  });

  streamDeckClient
    .getSettings()
    .then(({ settings }) => updateSettings(settings));
  streamDeckClient.didReceiveSettings.subscribe(({ payload }) =>
    updateSettings(payload.settings),
  );
})();
