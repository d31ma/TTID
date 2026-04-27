import tsPlugin from '@typescript-eslint/eslint-plugin'
import tsParser from '@typescript-eslint/parser'
import prettierConfig from 'eslint-config-prettier'

export default [
    {
        files: ['src/**/*.js', 'test/**/*.js'],
        languageOptions: {
            parser: tsParser,
            parserOptions: {
                project: './jsconfig.json'
            }
        },
        plugins: {
            '@typescript-eslint': tsPlugin
        },
        rules: {
            ...tsPlugin.configs['recommended'].rules,
            '@typescript-eslint/no-explicit-any': 'error',
            '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }]
        }
    },
    prettierConfig,
    {
        ignores: ['bin/**', 'node_modules/**', '**/*.d.ts']
    }
]
