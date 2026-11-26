import { z } from 'zod';
import { SKILL_KINDS, PROVIDERS, ROLES, SKILL_NAME_MAX, SKILL_DESC_MAX, SKILL_CATEGORIES } from '@x/core';

const skillFrontmatterBase = z.looseObject({
  name: z.string()
    .max(SKILL_NAME_MAX, `name must be at most ${SKILL_NAME_MAX} characters`)
    .regex(/^[a-z0-9-]+$/, 'only lowercase letters, numbers, and hyphens')
    .refine(s => !/claude|anthropic/i.test(s), 'must not contain "claude"/"anthropic"'),

  description: z.string()
    .min(1, 'description is required')
    .max(SKILL_DESC_MAX, `description must be at most ${SKILL_DESC_MAX} characters`),

  'disable-model-invocation': z.boolean().optional(),
  'user-invocable': z.boolean().optional(),

  'x-kind': z.enum(SKILL_KINDS),

  'x-tags': z.array(z.string()).default([]),

  'x-version': z.string()
    .regex(/^\d+\.\d+\.\d+$/, 'x-version must be semver (e.g. 1.2.3)')
    .default('0.1.0'),

  'x-roles': z.array(z.enum(ROLES)).default([]),

  'x-compatible': z.array(z.enum(PROVIDERS)).default([...PROVIDERS]),

  'x-category': z.enum(SKILL_CATEGORIES).optional(),
});

const knownKeys = new Set(Object.keys(skillFrontmatterBase.shape));

export const SkillFrontmatterSchema = skillFrontmatterBase
  .superRefine((data, ctx) => {
    for (const key of Object.keys(data)) {
      if (!key.startsWith('x-') && !knownKeys.has(key)) {
        ctx.addIssue({
          code: 'custom',
          message: `Unknown key does not start with "x-": "${key}"`,
          path: [key],
        });
      }
    }
  });

export type SkillFrontmatter = z.infer<typeof SkillFrontmatterSchema>;
