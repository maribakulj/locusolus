#!/bin/bash
# Attester un worker : une campagne de self-tests sous le mécanisme qu'il emploie.
#
# L'attestation lie un niveau prouvé à un hôte ET à un worker précis. Trois
# workers sur la même machine demandent donc trois campagnes : ce qui est prouvé
# pour l'un ne vaut pas pour l'autre, et c'est voulu — sans quoi enrôler un
# worker suffirait à hériter du confinement d'un voisin.
set -u
export LOCUS_PROBE_IMAGE=docker.io/library/alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
export LOCUS_EXECD_SECCOMP_PROFILE=$HOME/locus-etc/seccomp.json
export LOCUS_EXECD_PROBE_WORKSPACE=/srv/locus/travail
export LOCUS_EXECD_ATTESTATION_WORKER="$1"
export LOCUS_EXECD_ATTESTATION_OUT=$HOME/locus-etc/attestations.json
exec "$HOME/cible/debug/locus-execd" --certify --mechanism bubblewrap
