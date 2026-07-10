Feature: Continuous integration and PyPI packaging
  The project's quality gate (lint + full test suite) runs automatically on
  Linux and Windows for every push, and the package carries everything PyPI
  needs so a GitHub release publishes it as `pip install organize-by-extension`.
  PRD section 16, FR-42..FR-48.

  Scenario: The package version is single-sourced
    Then the pyproject version matches the package version

  Scenario: The package metadata is ready for PyPI
    Then the pyproject declares the distribution name "organize-by-extension"
    And the pyproject declares the metadata field "readme"
    And the pyproject declares the metadata field "license"
    And the pyproject declares the metadata field "classifiers"
    And the pyproject declares the metadata field "urls"
    And the pyproject declares the console script "file-organizer"

  Scenario: The repository carries an MIT license file
    Then the repository file "LICENSE" exists
    And the repository file "LICENSE" contains "MIT License"

  Scenario: CI runs lint and tests on Linux and Windows
    Then the repository file ".github/workflows/ci.yml" exists
    And the repository file ".github/workflows/ci.yml" contains "ubuntu-latest"
    And the repository file ".github/workflows/ci.yml" contains "windows-latest"
    And the repository file ".github/workflows/ci.yml" contains "ruff check"
    And the repository file ".github/workflows/ci.yml" contains "ruff format --check"
    And the repository file ".github/workflows/ci.yml" contains "pytest"

  Scenario: Releases publish to PyPI via trusted publishing
    Then the repository file ".github/workflows/publish.yml" exists
    And the repository file ".github/workflows/publish.yml" contains "release"
    And the repository file ".github/workflows/publish.yml" contains "id-token: write"
    And the repository file ".github/workflows/publish.yml" contains "twine check"
